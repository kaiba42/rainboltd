use crate::chain_clients::{MerchantPool, EscrowAccount};

use serde::{Serialize, Deserialize};
use bolt::channels::{ChannelState, ChannelToken};
use bolt::ped92::Commitment;
use pairing::bls12_381::Bls12;
use secp256k1::PublicKey;

#[derive(Serialize, Deserialize)]
pub struct EscrowFillMessage {
    pub merchant: String,
    pub customer_bls: PublicKey,
    pub wallet_commit: Vec<u8>, //Commitment<Bls12>,
}

#[derive(Serialize, Deserialize)]
pub struct EscrowLiquidityMessage {
    pub merchant_bls: PublicKey,
    pub channel_state: String, //ChannelState<Bls12>,
    pub channel_token: String, //ChannelToken<Bls12>,
}

// Should get imported from rainbolt_near_chain crate
#[derive(Serialize, Deserialize, Debug)]
pub struct NearMerchantPool {
    pub total: u128,
    pub available: u128,
    // address: String,
    pub bls_pub_key: String,
    // JSON
    pub channel_state: String,
    // JSON
    pub channel_token: String,
    pub escrows: Vec<NearEscrowAccount>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct NearEscrowAccount {
    pub amount: u128,
    pub customer: String,
    pub customer_bls: String,
    pub wallet_commit: Vec<u8>,
}

// TODO implement TryFrom for decoding message error handling
impl From<NearMerchantPool> for MerchantPool {
    fn from(near_merchant_pool: NearMerchantPool) -> MerchantPool {
        let NearMerchantPool { total, available, bls_pub_key, channel_state, channel_token, escrows } = near_merchant_pool;
        MerchantPool {
            total,
            available,
            bls_pub_key,
            channel_state: serde_json::from_slice(&base64::decode(&channel_state).unwrap()).unwrap(),
            channel_token: serde_json::from_slice(&base64::decode(&channel_token).unwrap()).unwrap(),
            escrows: escrows.into_iter().map(|near_escrow| EscrowAccount::from(near_escrow)).collect(),
        }
    }
}

impl From<NearEscrowAccount> for EscrowAccount {
    fn from(near_escrow: NearEscrowAccount) -> EscrowAccount {
        let NearEscrowAccount { amount, customer, customer_bls, wallet_commit } = near_escrow;
        EscrowAccount {
            amount,
            customer,
            customer_bls,
            wallet_commit,
        }
    }
}