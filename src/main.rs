use bolt::{
    handle_bolt_result,
    ped92::{Commitment, CommitmentProof},
    bidirectional::{
        init_customer,
        init_merchant,
        establish_customer_generate_proof,
        establish_merchant_issue_close_token,
        establish_merchant_issue_pay_token,
        establish_customer_final,
        generate_payment_proof,
        verify_payment_proof,
        generate_revoke_token,
        verify_revoke_token
    },
    channels::{
        ChannelState,
        CustomerState,
        MerchantState,
        ChannelToken
    }
};
use ff;
use rand;
use pairing::bls12_381::Bls12;
use warp::{
    self, 
    path, 
    reply,
    Filter,
    Reply
};
use serde::{Serialize, Deserialize};
use secp256k1;
use std::time::Instant;


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

#[derive(Serialize, Deserialize)]
pub struct MarketState {
    // pub channel_state: ChannelState<Bls12>,
    // pub merchant_state: MerchantState<Bls12>,
    // pub channel_token: ChannelToken<Bls12>,
    pub liquidity: u64,
    pub address: String,
}

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
}

pub struct OpenMarketState {
    pub last_index_price: f64,
}

pub struct OpenChannelMessage {
    pub customer_public_key: secp256k1::PublicKey,
    pub root_commitment: Commitment<Bls12>,
    pub root_commitment_proof: CommitmentProof<Bls12>,
    pub margin: i64,
    pub order_size: i64,
}

/*
OpenChannelMessage {
    customer_public_key: customer_state.pk_c.clone(), // send to merchant so they can update their channel state
    root_commitment,
    root_commitment_proof,
    margin,
    order_size,
}
*/

// TODO cure floating point precision error
fn compute_payment(market: &mut OpenMarketState, current_index_price: f64, position_size: i64) -> i64 {
    let change_in_price = current_index_price - market.last_index_price;
    let profit_or_loss = (position_size as f64) * change_in_price / market.last_index_price;
    market.last_index_price = current_index_price;
    profit_or_loss as i64
}

// fn recv_payment_req(maker_state: &mut MakerState, payment_message: PaymentMessage) {

// }

fn settle_contract(taker_state: &mut TakerState) {
    let rng = &mut rand::thread_rng();
    // Get open market data
    let mut open_market_state = OpenMarketState {
        last_index_price: 100.0_f64
    };
    let current_index_price = 110.0_f64;
    
    // compute payment
    let payment = compute_payment(&mut open_market_state, current_index_price, taker_state.order_size);

    // generate payment proof
    let (payment_proof, new_customer_state, pay_time) = measure_two_arg!(
        generate_payment_proof(
            rng, 
            &taker_state.channel_state, 
            &taker_state.customer_state, 
            payment
        )
    );
    println!(">> Time to generate payment proof: {} ms", pay_time);

    // ----- Send proof to merchant -----
    // Recv new close token
    let (new_close_token, verify_time) = measure_one_arg!(
        verify_payment_proof(
            rng, 
            &taker_state.channel_state, 
            &payment_proof, 
            &mut merchant_state
        )
    );
    println!(">> Time to verify payment proof: {} ms", verify_time);
    // -------- Send new_close_token to customer -------
    // Recv new close token
    // Create new revoke token and update customer state
    let revoke_token = generate_revoke_token(
        &taker_state.channel_state, 
        &mut taker_state.customer_state, 
        new_customer_state, 
        &new_close_token
    );

    // -------- Send revoke token to merchant ----- 
    // Recv new revoke token 
    // Create new pay token and update state
    let new_pay_token_result = verify_revoke_token(
        &revoke_token, 
        &mut merchant_state
    );
    let new_pay_token = handle_bolt_result!(new_pay_token_result);
    
    // --------- Send new pay token to customer --------
    // Recv and verify the pay token and update internal state
    assert!(taker_state.customer_state.verify_pay_token(&taker_state.channel_state, &new_pay_token.unwrap()));
}

// fn recv_open_channel_req(maker_state: &mut MakerState, open_channel_message: OpenChannelMessage) {

// }

fn send_open_channel_req(taker_state: &mut TakerState) {
    let rng = &mut rand::thread_rng();

    // TODO remove
    let (mut channel_token, merchant_state, channel_state) = init_merchant(rng, &mut taker_state.channel_state, "Merchant Bob");

    // send message to Merchant
    // receive closing token   
    let close_token = match establish_merchant_issue_close_token(
        rng, 
        &taker_state.channel_state, 
        &taker_state.root_commitment, 
        &taker_state.root_commitment_proof, 
        &taker_state.channel_id, 
        taker_state.initial_margin, 
        taker_state.order_size, 
        &merchant_state
    ) {
        Ok(token) => token.expect("valid close_token is empty"),
        Err(err) => panic!("Failed - bidirectional::establish_merchant_issue_close_token(): {}", err)
    };
    // validate token & update taker state
    assert!(taker_state.customer_state.verify_close_token(&taker_state.channel_state, &close_token));
    println!("Verified close token!");

    // receive payment token for pay protocol
    let pay_token = establish_merchant_issue_pay_token(
        rng, 
        &taker_state.channel_state, 
        &taker_state.root_commitment, 
        &merchant_state
    );
    // validate token & update taker state
    assert!(establish_customer_final(&mut taker_state.channel_state, &mut taker_state.customer_state, &pay_token));
    println!("Verified payment token!");
    println!("Channel established!");
}

fn place_order(initial_margin: i64, channel_state: ChannelState<Bls12>, mut channel_token: ChannelToken<Bls12>, order_size: i64) -> TakerState {
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
    
    let mut taker_state = TakerState {
        channel_id: channel_token.compute_channel_id(),
        channel_token,
        channel_state,
        customer_state,
        root_commitment,
        root_commitment_proof,
        initial_margin,
        order_size,
        available_margin: initial_margin,
    };

    send_open_channel_req(&mut taker_state);

    taker_state
}

fn get_market_state(address: String) -> impl Reply {
    let state = MarketState {
        // channel_state: ChannelState::new("Market Channel".to_string(), false),
        liquidity: 100,
        address
    };
    reply::json(&state)
}

fn main() {
    warp::serve(
        path!("hello" / String).map(|name| format!("Hello, {}!", name))
        .or(path!("state" / String).map(get_market_state))
    )
    .run(([127, 0, 0, 1], 3030));
}
