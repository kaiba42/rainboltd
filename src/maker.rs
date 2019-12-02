// External
use bolt::{
    handle_bolt_result,
    bidirectional::{
        init_merchant,
        establish_merchant_issue_close_token,
        establish_merchant_issue_pay_token,
        verify_payment_proof,
        verify_revoke_token
    },
    channels::{
        ChannelState,
        MerchantState,
        ChannelToken
    }
};
use ff;
use rand;
use pairing::bls12_381::Bls12;
use serde::{Serialize, Deserialize};
use std::time::Instant;
use http::header::{HeaderValue, CONTENT_TYPE};

// Internal
use crate::message::{
    OpenChannelRequest,
    OpenChannelResponse,
    PaymentRequest,
    PaymentResponse,
    GeneratePaymentTokenRequest,
    GeneratePaymentTokenResponse,
    OpenMarketState
};
use crate::math;
use crate::MarketData;

macro_rules! measure_one_arg {
    ($x: expr) => {
        {
            let s = Instant::now();
            let res = $x;
            let e = s.elapsed();
            (res, e.as_millis())
        };
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct MakerState {
    pub channel_id: Option<<Bls12 as ff::ScalarEngine>::Fr>,
    pub channel_token: ChannelToken<Bls12>,
    pub channel_state: ChannelState<Bls12>,
    pub merchant_state: MerchantState<Bls12>,
    // pub root_commitment: Commitment<Bls12>,
    // pub root_commitment_proof: CommitmentProof<Bls12>,
    pub initial_margin: i64,
    pub order_size: Option<i64>,
    pub available_margin: i64,
    pub market_data: Option<MarketData>,
    pub prev_market_data: Option<MarketData>
}

impl warp::Reply for MakerState {
    fn into_response(self) -> warp::reply::Response {
        let body = serde_json::to_vec(&self).expect("MakerState failed to serialize");
        let mut res = warp::reply::Response::new(body.into());
        res
            .headers_mut()
            .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        res
    }
}

pub trait Maker {
    fn init(initial_margin: i64) -> Self;
    fn place_order(&mut self);
    fn recv_open_channel_req(&mut self, req: OpenChannelRequest) -> OpenChannelResponse;
    fn recv_payment_req(&mut self, req: PaymentRequest) -> PaymentResponse;
    fn recv_generate_payment_token_req(&mut self, req: GeneratePaymentTokenRequest) -> GeneratePaymentTokenResponse;
}

impl Maker for MakerState {
    fn init(initial_margin: i64) -> Self {
        let rng = &mut rand::thread_rng();
        let mut channel_state = ChannelState::<Bls12>::new(String::from("Channel A -> B"), false);
        let (channel_token, merchant_state, channel_state) = init_merchant(rng, &mut channel_state, "Merchant Bob");
        
        MakerState {
            channel_id: None,
            channel_token,
            channel_state,
            merchant_state,
            // root_commitment,
            // root_commitment_proof,
            initial_margin,
            order_size: None,
            available_margin: initial_margin,
            market_data: None,
            prev_market_data: None,
        }
    }

    fn place_order(&mut self) {
        // TODO send channel_token, keys, etc. to Cosmos
    }

    fn recv_open_channel_req(&mut self, req: OpenChannelRequest) -> OpenChannelResponse {
        println!("Open Channel Request received!");
        let rng = &mut rand::thread_rng();
        let OpenChannelRequest {
            root_commitment,
            root_commitment_proof,
            customer_public_key,
            margin,
            order_size
        } = req;

        // Record order size
        self.order_size = Some(order_size);

        // Save customer public key and Generate channel id
        self.channel_token.set_customer_pk(&customer_public_key);
        self.channel_id = Some(self.channel_token.compute_channel_id());

        // receive closing token   
        let close_token = match establish_merchant_issue_close_token(
            rng, 
            &self.channel_state, 
            &root_commitment, 
            &root_commitment_proof, 
            &self.channel_id.expect("id was set earlier"), 
            margin, 
            order_size, 
            &self.merchant_state
        ) {
            Ok(token) => token.expect("valid close_token is empty"),
            Err(err) => panic!("Failed - bidirectional::establish_merchant_issue_close_token(): {}", err)
        };

        // receive payment token for pay protocol
        let pay_token = establish_merchant_issue_pay_token(
            rng, 
            &self.channel_state, 
            &root_commitment, 
            &self.merchant_state
        );

        // TODO send pay_token and close_token to client
        OpenChannelResponse {
            close_token,
            pay_token
        }
    }

    fn recv_payment_req(&mut self, req: PaymentRequest) -> PaymentResponse {
        let rng = &mut rand::thread_rng();
        let PaymentRequest {
            payment_proof
        } = req;
        
        // compute payment
        let market_data = self.market_data.clone().expect("must have market data");
        let prev_market_data = self.prev_market_data.clone().expect("must have market data");
        let position_size = self.order_size.clone().expect("Order exists if payment received");
        let payment = math::compute_payment(market_data, prev_market_data, position_size);
        // Verify amount
        if payment != payment_proof.amount { // TODO add some tolerance specified in the contract, e.g. a few cents of difference, can average values or dispute
            panic!("Payment expected {} received {}", payment, payment_proof.amount)
        }

        let (close_token, verify_time) = measure_one_arg!(
            verify_payment_proof(
                rng, 
                &self.channel_state, 
                &payment_proof, 
                &mut self.merchant_state
            )
        );
        println!(">> Time to verify payment proof: {} ms", verify_time);
        // -------- Send new_close_token to customer -------
        PaymentResponse {
            close_token
        }
    }

    fn recv_generate_payment_token_req(&mut self, req: GeneratePaymentTokenRequest) -> GeneratePaymentTokenResponse {
        // Recv new revoke token 
        let GeneratePaymentTokenRequest {
            revoke_token
        } = req;

        // Create new pay token and update state
        let new_pay_token_result = verify_revoke_token(
            &revoke_token, 
            &mut self.merchant_state
        );
        let payment_token = handle_bolt_result!(new_pay_token_result).expect("Payment token is Some()");
        // --------- Send new pay token to customer --------
        GeneratePaymentTokenResponse {
            payment_token
        }
    }
}
