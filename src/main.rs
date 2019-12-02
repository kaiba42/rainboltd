use bolt::{
    bidirectional::{
        init_merchant,
        ChannelcloseC,
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
use serde::{Serialize, Deserialize};
use reqwest::Client;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use std::io::{Read, Write, BufReader};
use std::path::Path;
use std::fs::File;
use lazy_static::lazy_static;
use clap::{Arg, App, SubCommand, crate_version};

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
    },
    chain_clients::{
        ChainClient, 
        ChainClients, 
        ChainError
    },
    near::NearChainClient,
    config::{RainboltdConfig, load_config},
    MarketData,
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

fn init_maker_state(initial_margin: i64) -> MakerState {
    let mut maker = MAKER_SLOT.lock().expect("Maker is not poisoned");
    if maker.is_some() {
        println!("Maker already exists");
        return maker.clone().unwrap()
    };
    
    println!("Creating a new Maker!");
    let maker_state = maker.get_or_insert(
        MakerState::init(initial_margin)
    );
    // Store to disk
    // FIXME should get encrypted and stored in a configurable location
    save_state(&maker_state, "./maker_state.json").expect("Could not save MakerState to disk");
    maker_state.clone()
}

// Load maker_state from disk
fn load_maker_state() -> Result<MakerState, std::io::Error> {
    // FIXME should get unencrypted and loaded from a configurable location
    let mut file = std::fs::File::open("./maker_state.json")?;
    // We need to read to string to enforce well-formed utf8 because the secp256k1 crate expects
    // an &str for SecretKey and does not do a str::FromStr::from_str(hex) like with PublicKey
    let mut string = String::new();
    file.read_to_string(&mut string)?;
    Ok(serde_json::from_str(&string).expect("Could not parse MakerState"))
}

// Load taker_state from disk
fn load_taker_state() -> Result<TakerState, std::io::Error> {
    // FIXME should get unencrypted and loaded from a configurable location
    let mut file = std::fs::File::open("./taker_state.json")?;
    // We need to read to string to enforce well-formed utf8 because the secp256k1 crate expects
    // an &str for SecretKey and does not do a str::FromStr::from_str(hex) like with PublicKey
    let mut string = String::new();
    file.read_to_string(&mut string)?;
    Ok(serde_json::from_str(&string).expect("Could not parse TakerState"))
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
        maker_order_id,
        chain
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

fn save_state<T: Serialize, P: AsRef<Path>>(state: &T, name: P) -> Result<(), std::io::Error> {
    let mut file = std::fs::File::create(name)?;
    file.write(serde_json::to_string(state).unwrap().as_bytes())?;
    Ok(())
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

fn get_close_message(taker_slot: Arc<Mutex<Option<TakerState>>>) -> ChannelcloseC<Bls12> {
    let mut taker = TAKER_SLOT.lock().expect("Taker is not poisoned");
    taker
        .as_mut()
        .map(|taker| taker.get_close_message())
        .expect("Taker exists")
}

lazy_static! {
    static ref TAKER_SLOT: Arc<Mutex<Option<TakerState>>> = Arc::new(Mutex::new(None));
    static ref MAKER_SLOT: Arc<Mutex<Option<MakerState>>> = Arc::new(Mutex::new(None));
    static ref CHAINCLIENTS: Arc<Mutex<HashMap<&'static str, Arc<Box<dyn ChainClient>>>>> = Arc::new(Mutex::new(HashMap::new()));
    static ref CLIENT: Client = Client::new();
    static ref CONFIG_PATH: Mutex<Option<String>> = Mutex::new(None);
    static ref CONFIG: Arc<RainboltdConfig> = Arc::new(load_config(CONFIG_PATH.lock().unwrap().as_ref()));

    static ref OPEN_CHANNEL_URL: String = format!("{}/maker/openChannel", CONFIG.channel_ip);
    static ref RECV_PAY_URL: String = format!("{}/maker/recvPay", CONFIG.channel_ip);
    static ref PAYMENT_TOKEN_URL: String = format!("{}/maker/paymentToken", CONFIG.channel_ip);
}

async fn start_server() {
    // let taker_slot = Arc::new(Mutex::new(None));
    // let maker_slot = Arc::new(Mutex::new(None));

    let state = path!(String / "state").map(|id| -> String {
        id
    });

    let init_maker = path!("init" / i64 / String).and_then(|initial_margin, chain: String| async move {
        let chain_client = match CHAINCLIENTS.lock().expect("ChainClients poisoned")
            .get(chain.clone().to_lowercase().as_str()) {
             // TODO should check chain if funds are already escrowed
            Some(client) => client.clone(),
            None => return Err(warp::reject::custom(ChainError::ChainNotAvailable)),
        };

        let maker_state = init_maker_state(initial_margin);
        println!("Escrowing amount {} on {}", initial_margin, chain.to_uppercase());
        match chain_client.sign_and_send_liquidity_msg(&maker_state, initial_margin as u128).await {
            Ok(_) => Ok::<MakerState, Rejection>(maker_state),
            Err(err_string) => Err(warp::reject::custom(ChainError::ChainErr(err_string))),
        }
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
        // TODO check that OrderRequest.order_size is > 0
        .and_then(|order_request: OrderRequest| async move {
            // Check if the maker liquidity exists on chain
            let chain_client = match CHAINCLIENTS.lock().expect("ChainClients is poisoned").get(order_request.chain.clone().as_str()) {
                Some(client) => client.clone(),
                None => return Err(warp::reject::custom(ChainError::ChainNotAvailable)),
            };
            let merchants = chain_client.show_liquidity().await.expect("Could not get MerchantPools");
            let mut taker_state = None;

            for (id, merchant_pool) in merchants {
                if id == order_request.maker_order_id {
                    let mut taker = TAKER_SLOT.lock().expect("Taker is not poisoned");
                    if taker.is_none() {
                        println!("Creating a new Taker!");
                        taker_state = Some(taker.get_or_insert(
                            TakerState::init(
                                order_request.initial_margin.clone(), 
                                order_request.order_size.clone(), 
                                merchant_pool.channel_state, 
                                merchant_pool.channel_token
                            )
                        ).clone());
                        save_state(taker_state.as_ref().unwrap(), "./taker_state.json").expect("Could not save TakerState to disk");
                        break;
                    } else {
                        println!("Taker already has an order");
                        return Ok(taker.clone().unwrap())
                    }
                }
            }

            match taker_state {
                // Escrow the amount on chain
                Some(taker_state) => match chain_client.sign_and_send_fill_msg(
                    &taker_state, 
                    order_request.maker_order_id, 
                    order_request.order_size as u128
                ).await {
                    Ok(_) => {
                        // TODO get the maker ip address via a relay node or other source of pub_key to ip mapping
                        // Or use Hub & Spoke third party nodes
                        let res: OpenChannelResponse = CLIENT.post(&*OPEN_CHANNEL_URL)
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
                    },
                    Err(err_string) => Err(warp::reject::custom(ChainError::ChainErr(err_string)))
                },
                None => Err(warp::reject::custom(ChainError::MakerNotFound))
            }
        });
    
    // TODO Update maker and taker state on disk   
    let send_payment = path!("pay")
        // .and(warp::body::json())
        .and_then(|| async {
            let send_payment_req = get_taker_payment_req(TAKER_SLOT.clone());
            println!("Sending payment request: {}", send_payment_req.payment_proof.amount);
            let send_payment_res: PaymentResponse = CLIENT.post(&*RECV_PAY_URL)
                .json(&send_payment_req)
                .send()
                .await
                .expect("send payment request failed")
                .json()
                .await
                .expect("send payment response parsing failed");
            
            let generate_payment_token_req = update_taker_state_with_payment_res(TAKER_SLOT.clone(), send_payment_res);
            let generate_payment_token_res: GeneratePaymentTokenResponse = CLIENT.post(&*PAYMENT_TOKEN_URL)
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

    let close = path!("close" / String / String)
        .and_then(|merchant: String, chain: String| async move {
            let chain_client = match CHAINCLIENTS.lock().expect("ChainClients is poisoned").get(chain.clone().as_str()) {
                Some(client) => client.clone(),
                None => return Err(warp::reject::custom(ChainError::ChainNotAvailable)),
            };
            // TODO check that the escrow exists
            // let merchants = chain_client.show_liquidity().await.expect("Could not get MerchantPools");
            let close_message = get_close_message(TAKER_SLOT.clone());
            
            match chain_client.close_escrow_taker(merchant, close_message).await {
                Ok(res) => Ok::<String, warp::Rejection>(res),
                Err(err) => Err(warp::reject::custom(ChainError::ChainErr(err)))
            }
        });

    let taker_path = path!("taker")
        .and(
            take_order
            .or(send_payment)
            .or(close)
        );

    let market_path = path!("marketData")
        .and(warp::body::json())
        .map(|req: MarketData| {
            println!("Got new market data! {:?}", req);
            let taker_slot = TAKER_SLOT.clone();
            let maker_slot = MAKER_SLOT.clone();
            let mut maybe_taker = taker_slot.lock().expect("Taker is not poisoned during market data feed");
            let mut maybe_maker = maker_slot.lock().expect("Maker is not poisoned during market data feed");
            maybe_taker.as_mut().map(|taker| {
                taker.prev_market_data = taker.market_data.clone();
                taker.market_data = Some(req.clone());
                println!("Updated Taker MarketData!");
            });
            maybe_maker.as_mut().map(|maker| {
                maker.prev_market_data = maker.market_data.clone();
                maker.market_data = Some(req);
                println!("Updated Maker MarketData!");
            });
            "Success".to_string()
        });
    
    println!("Starting rainboltd");
    warp::serve(
        warp::post().and(maker_path.or(taker_path).or(market_path))
    )
    .run(([127, 0, 0, 1], 3031)).await;
}

#[tokio::main]
async fn main() {
    let matches = App::new("rainboltd")
        .version(crate_version!())
        .about("Rainbolt network server daemon")
        .arg(Arg::with_name("config")
            .short("c")
            .long("config")
            .value_name("path to json config")
            .help("configuration for chain keys"))
        .arg(Arg::with_name("load")
            .short("l")
            .long("load")
            .help("load state from json"))
        .arg(Arg::with_name("near_key")
            .short("nk")
            .long("near_key")
            .value_name("near secret key for testing")
            .help("NEAR ed25519 secret key for testing"))
        .arg(Arg::with_name("near_account_id")
            .short("nid")
            .short("near_account_id")
            .value_name("near account id"))
        .get_matches();

    if let Some(config_path) = matches.value_of("config") {
        CONFIG_PATH.lock().unwrap().replace(config_path.to_string());
        match &CONFIG.chains {
            Some(chains) => {
                let mut chain_clients = CHAINCLIENTS.lock().expect("Synchronous call");
                chains.iter().for_each(|chain_config| {
                    match chain_config.chain_id.as_str() {
                        "near" | "Near" | "NEAR" => {
                            let near_client = NearChainClient::from_secret_key(
                                CLIENT.clone(), 
                                chain_config.account_id.clone(), 
                                chain_config.secret_key.clone()
                            );
                            chain_clients.insert("near", Arc::new(Box::new(near_client)));
                        }
                        _ => panic!("NEAR is the only supported chain"),
                    }
                });
            },
            None => println!("No chains found in config file"),
        }
    };

    if matches.is_present("load") {
        match load_maker_state() {
            Ok(maker_state) => {
                println!("Loaded MakerState");
                MAKER_SLOT.lock().unwrap().replace(maker_state);
            },
            Err(err) => eprintln!("Could not load MakerState: {}", err)
        }

        match load_taker_state() {
            Ok(taker_state) => {
                println!("Loaded TakerState");
                TAKER_SLOT.lock().unwrap().replace(taker_state);
            },
            Err(err) => eprintln!("Could not load TakerState: {}", err)
        }
    }

    if let Some(secret_key) = matches.value_of("near_key") {
        let account_id = matches.value_of("near_account_id").expect("Account Id required for NEAR secrety Key");
        let near_client = NearChainClient::from_secret_key(
            CLIENT.clone(), 
            account_id.to_string(), 
            serde_json::from_str(secret_key).expect("Could not parse near secret key")
        );
        CHAINCLIENTS.lock().expect("Synchronous call").insert("near", Arc::new(Box::new(near_client)));
    }

    start_server().await
}
