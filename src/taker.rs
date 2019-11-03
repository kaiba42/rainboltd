// External
use bolt::{
    ped92::{Commitment, CommitmentProof},
    bidirectional::{
        RevokeToken,
        init_customer,
        establish_customer_generate_proof,
        establish_customer_final,
        generate_payment_proof,
        generate_revoke_token,
    },
    channels::{
        ChannelState,
        CustomerState,
        ChannelToken
    }
};
use ff;
use rand;
use pairing::bls12_381::Bls12;
use serde::{Serialize, Deserialize};
use std::time::Instant;
use reqwest::r#async::Client;
use http::header::{HeaderValue, CONTENT_TYPE};
// use futures::future::Future;

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


// macro_rules! measure_one_arg {
//     ($x: expr) => {
//         {
//             let s = Instant::now();
//             let res = $x;
//             let e = s.elapsed();
//             (res, e.as_millis())
//         };
//     }
// }

macro_rules! measure_two_arg {
    ($x: expr) => {
        {
            let s = Instant::now();
            let (res1, res2) = $x;
            let e = s.elapsed();
            (res1, res2, e.as_millis())
        };
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct TakerState {
    pub channel_id: <Bls12 as ff::ScalarEngine>::Fr,
    pub channel_token: ChannelToken<Bls12>,
    pub channel_state: ChannelState<Bls12>,
    pub customer_state: CustomerState<Bls12>,
    pub root_commitment: Commitment<Bls12>,
    pub root_commitment_proof: CommitmentProof<Bls12>,
    pub initial_margin: i64,
    pub order_size: i64,
    pub available_margin: i64,
    pub new_customer_state: Option<CustomerState<Bls12>>,
    pub revoke_token: Option<RevokeToken>
}

impl warp::Reply for TakerState {
    fn into_response(self) -> warp::reply::Response {
        let body = serde_json::to_vec(&self).expect("TakerState failed to serialize");
        let mut res = warp::reply::Response::new(body.into());
        res
            .headers_mut()
            .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        res
    }
}

pub trait Taker {
    fn init(initial_margin: i64, order_size: i64, channel_state: ChannelState<Bls12>, channel_token: ChannelToken<Bls12>) -> Self;
    fn take_order(&mut self);
    fn send_open_channel_req(&self) -> OpenChannelRequest;
    fn recv_open_channel_res(&mut self, res: OpenChannelResponse);
    fn send_payment_req(&mut self) -> PaymentRequest;
    fn recv_payment_res(&mut self, res: PaymentResponse);
    fn send_generate_payment_token_req(&mut self);
    fn recv_generate_payment_token_res(&mut self, res: GeneratePaymentTokenResponse);
}   

impl Taker for TakerState {
    fn init(initial_margin: i64, order_size: i64, channel_state: ChannelState<Bls12>, mut channel_token: ChannelToken<Bls12>) -> Self {
        let rng = &mut rand::thread_rng();
        let mut customer_state = init_customer(
            rng, 
            &mut channel_token, // Pub key of merchant, updated with Pub key of customer, Bls keys
            initial_margin, // initial balance of customer 
            order_size, // initial balance of the merchant 
            "YouKnowNothing"
        );

        let (root_commitment, root_commitment_proof, est_time) = measure_two_arg!(
            establish_customer_generate_proof(
                rng, 
                &mut channel_token, 
                &mut customer_state
            )
        );
        println!(">> Time to generate proof for establish: {} ms", est_time);
        
        TakerState {
            channel_id: channel_token.compute_channel_id(),
            channel_token,
            channel_state,
            customer_state,
            new_customer_state: None,
            root_commitment,
            root_commitment_proof,
            initial_margin,
            order_size,
            available_margin: initial_margin,
            revoke_token: None,
        }
    }

    fn take_order(&mut self) {
        self.send_open_channel_req();
    }

    fn send_open_channel_req(&self) -> OpenChannelRequest {
        // send message to Merchant
        let req = OpenChannelRequest {
            customer_public_key: self.customer_state.pk_c,
            root_commitment: self.root_commitment.clone(),
            root_commitment_proof: self.root_commitment_proof.clone(),
            margin: self.initial_margin,
            order_size: self.order_size,
        };

        // TODO non blocking send
        println!("Open Channel Request sent!");
        req
    }

    fn recv_open_channel_res(&mut self, res: OpenChannelResponse) {
        println!("Open Channel Response received!");
        let OpenChannelResponse {
            close_token,
            pay_token
        } = res;

        // validate token & update taker state
        assert!(self.customer_state.verify_close_token(&self.channel_state, &close_token));
        println!("verified close token!");

        // validate token & update taker state
        assert!(establish_customer_final(&mut self.channel_state, &mut self.customer_state, &pay_token));
        println!("verified payment token!");
        println!("Channel established!");
    }

    fn send_payment_req(&mut self) -> PaymentRequest {
        let rng = &mut rand::thread_rng();
        // Get open market data
        let mut open_market_state = OpenMarketState {
            last_index_price: 100.0_f64
        };
        let current_index_price = 110.0_f64;
        
        // compute payment
        let payment = math::compute_payment(&mut open_market_state, current_index_price, self.order_size);

        // generate payment proof
        let (payment_proof, new_customer_state, pay_time) = measure_two_arg!(
            generate_payment_proof(
                rng, 
                &self.channel_state, 
                &self.customer_state, 
                payment
            )
        );
        self.new_customer_state = Some(new_customer_state);
        println!(">> Time to generate payment proof: {} ms", pay_time);

        // TODO ----- Send proof to merchant -----
        let req = PaymentRequest {
            payment_proof
        };
        println!("Payment Request sent!");
        req
    }

    fn recv_payment_res(&mut self, res: PaymentResponse) {
        println!("Payment Response received!");
        // Recv new close token
        let PaymentResponse {
            close_token
        } = res;

        // Create new revoke token and update customer state
        self.revoke_token = Some(generate_revoke_token(
            &self.channel_state, 
            &mut self.customer_state, 
            self.new_customer_state.clone().expect("new_customer_state is defined in response to a payment"), 
            &close_token
        ));
        println!("generated revoke token!");

        // -------- Send revoke token to merchant ----- 
        self.send_generate_payment_token_req();
    }

    fn send_generate_payment_token_req(&mut self) {
        let req = GeneratePaymentTokenRequest {
            revoke_token: self.revoke_token.clone().expect("Revoke token must be Some() to generate a payment token")
        };
        // TODO -------- Send revoke token to merchant ----- 
        println!("Generate Payment Token Request sent!");
    }

    fn recv_generate_payment_token_res(&mut self, res: GeneratePaymentTokenResponse) {
        println!("Generate Payment Token Response received!");
        // Recv and verify the pay token and update internal state
        let GeneratePaymentTokenResponse {
            payment_token
        } = res;
        assert!(self.customer_state.verify_pay_token(&self.channel_state, &payment_token));
        println!("Generated payment_token is valid!");
    }
}
