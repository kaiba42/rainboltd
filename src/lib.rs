#![allow(unused_imports)]

pub mod message;
pub mod taker;
pub mod maker;
pub mod math;
// pub mod price_feed;

use serde::{Serialize, Deserialize};
use std::fmt::Debug;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MarketPrice {
    pub usd: i64
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MarketData {
    pub bitcoin: MarketPrice,
    pub cosmos: MarketPrice
}
pub mod cosmos;
pub mod cosmos;
pub mod near;
