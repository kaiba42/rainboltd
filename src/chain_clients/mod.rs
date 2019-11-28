use crate::maker::{MakerState};
use crate::taker::{TakerState};

use serde::{Serialize, Deserialize};
use bolt::channels::{ChannelState, ChannelToken};
use bolt::ped92::Commitment;
use pairing::bls12_381::Bls12;
use async_trait::async_trait;

#[async_trait]
pub trait ChainClient {
    async fn sign_and_send_liquidity_msg(&self, maker_state: &MakerState, amount: u128) -> Result<String, String>;
    /// `merchant` is an account id or public key used to select the merchant liquidity pool to fill
    async fn sign_and_send_fill_msg(&self, taker_state: &TakerState, merchant: String, amount: u128) -> Result<String, String>;
    async fn show_liquidity(&self) -> Result<Vec<MerchantPool>, String>;
}

// TODO Should get imported from rainbolt_near_chain crate
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