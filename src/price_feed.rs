use warp;
use std::sync::{Arc, Mutex};
use tokio::timer::Interval;
use std::time::{Duration, Instant};
use serde::{Serialize, Deserialize};
use futures01::stream::Stream;
use futures::stream::StreamExt;
use futures::stream::Stream;
use futures::compat::Stream01CompatExt;
// use futures::compat::Future01CompatExt;
use reqwest::Client;

// Internal
use crate::{
    taker::TakerState,
    maker::MakerState
};

#[derive(Serialize, Deserialize)]
struct MarketPrice {
    pub usd: f64
}

#[derive(Serialize, Deserialize)]
struct MarketData {
    pub bitcoin: MarketPrice,
    pub cosmos: MarketPrice
}

const COINGECKO_URI: &'static str = "http://api.coingecko.com/api/v3/simple/price?ids=cosmos,bitcoin&vs_currencies=usd";

pub fn stream_prices_to_state(taker_slot: Arc<Mutex<Option<TakerState>>>, maker_slot: Arc<Mutex<Option<MakerState>>>) {
    // TODO start at nearest utc minute or something

    warp::spawn(
        Interval::new(Instant::now(), Duration::from_secs(60))
            .for_each(|instant| {
                println!("Requesting new market data!");
                // let market_data = Client::new()
                //     .get(COINGECKO_URI)
                //     .send()
                //     .await
                //     .expect("Market data fetch failed")
                //     .json()
                //     .await
                //     .expect("Market data parsing failed");
                
                // if let Some(mut taker) = taker_slot.clone().lock().expect("Taker is poisoned") { taker.market_data = market_data };
                // if let Some(mut maker) = maker_slot.clone().lock().expect("Maker is poisoned") { maker.market_data = market_data };
                println!("Updated market data!");
                Ok(())
            })
            .compat()
    );
}