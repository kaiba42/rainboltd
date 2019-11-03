use bolt::{
    bidirectional::{
        init_merchant,
    },
    channels::{
        ChannelState,
        ChannelToken
    }
};
use pairing::bls12_381::Bls12;
use rand;
// use tokio::prelude::*;
use warp::{
    self, 
    path, 
    reply,
    Filter,
    Reply,
    reject::Rejection
};
// use futures::future::ok;
// use async_std::future;
use reqwest::Client;
use std::sync::{Arc, Mutex};
use lazy_static::lazy_static;

// Internal
use rainboltd::{
    taker::{
        TakerState, 
        Taker
    },
    maker::{
        Maker,
        MakerState
    },
    message::{
        OrderRequest,
        OpenChannelRequest,
        OpenChannelResponse,
        PaymentRequest,
        PaymentResponse,
        GeneratePaymentTokenRequest,
        GeneratePaymentTokenResponse
    }
};

// fn get_market_state(address: String) -> impl Reply {
//     let state = MarketState {
//         // channel_state: ChannelState::new("Market Channel".to_string(), false),
//         liquidity: 100,
//         address
//     };
//     reply::json(&state)
// }

fn recv_generate_payment_token_req(req: GeneratePaymentTokenRequest, maker_slot: Arc<Mutex<Option<MakerState>>>) -> GeneratePaymentTokenResponse {
    let mut maybe_maker = maker_slot.lock().expect("Maker is not poisoned");
    maybe_maker.as_mut().map(|maker| maker.recv_generate_payment_token_req(req)).expect("maker exists")
}

fn recv_payment_req(req: PaymentRequest, maker_slot: Arc<Mutex<Option<MakerState>>>) -> PaymentResponse {
    let mut maybe_maker = maker_slot.lock().expect("Maker is not poisoned");
    maybe_maker.as_mut().map(|maker| maker.recv_payment_req(req)).expect("maker exists")
}

fn open_channel_req(req: OpenChannelRequest, maker_slot: Arc<Mutex<Option<MakerState>>>) -> OpenChannelResponse {
    let mut maybe_maker = maker_slot.lock().expect("Maker is not poisoned");
    maybe_maker.as_mut().map(|maker| maker.recv_open_channel_req(req)).expect("maker exists")
}

fn init_maker_state(initial_margin: i64, maker_slot: Arc<Mutex<Option<MakerState>>>) -> impl Reply {
    let mut maker = maker_slot.lock().expect("Maker is not poisoned");
    if maker.is_none() {
        println!("Creating a new Maker!")
    };
    reply::json(
        maker.get_or_insert(
            MakerState::init(initial_margin)
        )
    )
}

fn get_channel_for_maker_order_id(maker_order_id: String) -> (ChannelState<Bls12>, ChannelToken<Bls12>) {
    let rng = &mut rand::thread_rng();
    let mut channel_state = ChannelState::<Bls12>::new(String::from("Channel A -> B"), false);
    let (channel_token, merchant_state, channel_state) = init_merchant(rng, &mut channel_state, "Merchant Bob");
    (channel_state, channel_token)
}

// fn order(req: OrderRequest, taker_slot: Arc<Mutex<Option<TakerState>>>, maker_slot: Arc<Mutex<Option<MakerState>>>) -> impl Future<Item=TakerState, Error=Rejection> {
fn order(req: OrderRequest, taker_slot: Arc<Mutex<Option<TakerState>>>, maker_slot: Arc<Mutex<Option<MakerState>>>) -> TakerState {
    let mut taker = taker_slot.lock().expect("Taker is not poisoned");
    if taker.is_none() {
        println!("Creating a new Taker!");
    } else {
        println!("Taker already has an order");
        return taker.clone().unwrap();
    };

    let OrderRequest {
        initial_margin,
        order_size,
        maker_order_id
    } = req;

    // let (channel_state, channel_token) = get_channel_for_maker_order_id(maker_order_id);
    let maybe_maker = maker_slot
        .lock()
        .expect("Order failed. Maker is poisoned");
    let channel_state = maybe_maker.as_ref().map(|maker| maker.channel_state.clone()).expect("maker exists");
    let channel_token = maybe_maker.as_ref().map(|maker| maker.channel_token.clone()).expect("maker exists");
    drop(maybe_maker);

    let taker_state = taker.get_or_insert(
        TakerState::init(
            initial_margin,
            order_size,
            channel_state,
            channel_token
        )
    );
    // let open_channel_req = taker_state.send_open_channel_req();

    //     .expect("open channel request failed")
    //     .json()
    //     .await
    //     .expect("open channel response parsing failed");
        
    // taker_state.recv_open_channel_res(res);
    // ok(taker_state.clone())
    taker_state.clone()
}

fn get_taker_payment_req(taker_slot: Arc<Mutex<Option<TakerState>>>) -> PaymentRequest {
    let mut maybe_taker = taker_slot.lock().expect("Taker is not poisoned");
    maybe_taker
        .as_mut()
        .map(|taker| taker.send_payment_req())
        .expect("taker exists")
}

fn update_taker_state_with_payment_res(taker_slot: Arc<Mutex<Option<TakerState>>>, send_payment_res: PaymentResponse) -> GeneratePaymentTokenRequest {
    let mut maybe_taker = taker_slot.lock().expect("Taker is not poisoned");
    maybe_taker
        .as_mut()
        .map(|taker| {
            taker.recv_payment_res(send_payment_res);
            taker.send_generate_payment_token_req()
        })
        .expect("taker exists")
}

lazy_static! {
    static ref TAKER_SLOT: Arc<Mutex<Option<TakerState>>> = Arc::new(Mutex::new(None));
    static ref MAKER_SLOT: Arc<Mutex<Option<MakerState>>> = Arc::new(Mutex::new(None));
}

#[tokio::main]
async fn main() {
    // let taker_slot = Arc::new(Mutex::new(None));
    // let maker_slot = Arc::new(Mutex::new(None));

    let state = path!(String / "state").map(|id| -> String {
        id
    });

    let init_maker_slot = MAKER_SLOT.clone();
    let init_maker = path!("init" / i64).map(move |initial_margin| {
        init_maker_state(initial_margin, init_maker_slot.clone())
    });

    let open_channel_maker_slot = MAKER_SLOT.clone();
    let open_channel = path!("openChannel")
        .and(warp::body::json())
        .map(move |req: OpenChannelRequest| {
            let res = open_channel_req(req, open_channel_maker_slot.clone());
            reply::json(&res)
        });
    
    let recv_pay_maker_slot = MAKER_SLOT.clone();
    let recv_pay = path!("recvPay")
        .and(warp::body::json())
        .map(move |req: PaymentRequest| {
            let res = recv_payment_req(req, recv_pay_maker_slot.clone());
            reply::json(&res)
        });

    let get_payment_token_maker_slot = MAKER_SLOT.clone();
    let get_payment_token = path!("paymentToken")
        .and(warp::body::json())
        .map(move |req: GeneratePaymentTokenRequest| {
            let res = recv_generate_payment_token_req(req, get_payment_token_maker_slot.clone());
            reply::json(&res)
        });

    let maker_path = path!("maker")
        .and(
            init_maker
            .or(open_channel)
            .or(recv_pay)
            .or(get_payment_token)
        );

    // let take_order_taker_slot = taker_slot.clone();
    // let take_order_maker_slot = maker_slot.clone();
    let take_order = path!("order")
        .and(warp::body::json())
        .and_then(|order_request: OrderRequest| async {
            let taker_state = order(order_request, TAKER_SLOT.clone(), MAKER_SLOT.clone());
            let client = Client::new();
            let res: OpenChannelResponse = client.post("http://localhost:3030/maker/openChannel")
                .json(&taker_state.send_open_channel_req())
                .send()
                .await
                .expect("open channel request failed")
                .json()
                .await
                .expect("open channel response parsing failed");

            let another_taker_clone = TAKER_SLOT.clone();
            let mut maybe_taker = another_taker_clone.lock().expect("Taker is not poisoned");
            let taker_updated_state = maybe_taker
                .as_mut()
                .map(|taker| {
                    taker.recv_open_channel_res(res);
                    taker
                })
                .expect("taker exists");

            Ok::<TakerState, warp::Rejection>(taker_updated_state.clone())
        });

    let send_payment = path!("pay")
        // .and(warp::body::json())
        .and_then(|| async {
            let send_payment_req = get_taker_payment_req(TAKER_SLOT.clone());
            println!("Sending payment request: {}", send_payment_req.payment_proof.amount);
            let client = Client::new();
            let send_payment_res: PaymentResponse = client.post("http://localhost:3030/maker/recvPay")
                .json(&send_payment_req)
                .send()
                .await
                .expect("send payment request failed")
                .json()
                .await
                .expect("send payment response parsing failed");
            
            let generate_payment_token_req = update_taker_state_with_payment_res(TAKER_SLOT.clone(), send_payment_res);
            let generate_payment_token_res: GeneratePaymentTokenResponse = client.post("http://localhost:3030/maker/paymentToken")
                .json(&generate_payment_token_req)
                .send()
                .await
                .expect("send generate payment token request failed")
                .json()
                .await
                .expect("send generate payment token parsing failed");
            
            let another_taker_clone = TAKER_SLOT.clone();
            let mut maybe_taker = another_taker_clone.lock().expect("Taker is not poisoned");
            let taker_updated_state = maybe_taker
                .as_mut()
                .map(|taker| {
                    taker.recv_generate_payment_token_res(generate_payment_token_res);
                    taker
                })
                .expect("taker exists");

            Ok::<TakerState, warp::Rejection>(taker_updated_state.clone())
        });

    let taker_path = path!("taker")
        .and(
            take_order
            .or(send_payment)
        );

    warp::serve(
        warp::post2().and(maker_path.or(taker_path))
    )
    .run(([127, 0, 0, 1], 3030)).await;
}
