use crate::maker::{MakerState};
use crate::taker::{TakerState};

use serde::{Serialize, Deserialize};
use bolt::channels::{ChannelState, ChannelToken};
use bolt::bidirectional::ChannelcloseC;
use bolt::ped92::Commitment;
use pairing::bls12_381::Bls12;
use async_trait::async_trait;

use std::error::Error;
use std::fmt;

pub type ChainClients = std::collections::HashMap<&'static str, Box<dyn ChainClient>>;

#[async_trait]
pub trait ChainClient: Sync + Send {
    async fn sign_and_send_liquidity_msg(&self, maker_state: &MakerState, amount: u128) -> Result<String, String>;
    /// `merchant` is an account id or public key used to select the merchant liquidity pool to fill
    async fn sign_and_send_fill_msg(&self, taker_state: &TakerState, merchant: String, amount: u128) -> Result<String, String>;
    async fn show_liquidity(&self) -> Result<Vec<(String, MerchantPool)>, String>;
    async fn close_escrow_taker(&self, merchant: String, close_message: ChannelcloseC<Bls12>) -> Result<String, String>;
}

#[derive(Serialize, Deserialize, Debug)]
pub enum ChainError {
    ChainNotAvailable,
    ChainErr(String),
    MakerNotFound,
}

impl fmt::Display for ChainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChainError::ChainNotAvailable => write!(f, "Chain not available"),
            ChainError::ChainErr(error) => write!(f, "Chain error: {}", error),
            ChainError::MakerNotFound => write!(f, "Maker not found on chain"),
        }
    }
}

impl Error for ChainError {}
impl warp::reject::Reject for ChainError {}

#[derive(Serialize, Deserialize)]
pub struct MerchantPool {
    pub total: u128,
    pub available: u128,
    pub bls_pub_key: String,
    pub channel_state: ChannelState<Bls12>,
    pub channel_token: ChannelToken<Bls12>,
    pub escrows: Vec<EscrowAccount>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct EscrowAccount {
    pub amount: u128,
    pub customer: String,
    pub customer_bls: String,
    pub wallet_commit: Vec<u8>,
}