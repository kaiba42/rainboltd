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
pub struct MerchantPool {
    pub total: u128,
    pub available: u128,
    // address: String,
    pub bls_pub_key: String,
    // JSON
    pub channel_state: String,
    // JSON
    pub channel_token: String,
    pub escrows: Vec<EscrowAccount>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct EscrowAccount {
    pub amount: u128,
    pub customer: String,
    pub customer_bls: String,
    pub wallet_commit: Vec<u8>,
}

// #[derive(Serialize, Deserialize, Debug)]
// #[serde(rename_all = "camelCase")]
// struct NearStatusRequest {
//     id: String,
//     jsonrpc: String,
//     method: String,
//     params: Vec<u8>,
// }

// #[derive(Deserialize, Debug)]
// struct NearSyncInfo {
//     latest_block_hash: String,
//     latest_block_height: u64,
//     latest_block_time: String,
//     latest_state_root: String,
//     syncing: bool,
// }

// #[derive(Deserialize, Debug)]
// struct NearValidatorInfo {
//     account_id: String,
//     is_slashed: bool,
// }

// #[derive(Deserialize, Debug)]
// struct NearVersionInfo {
//     build: String,
//     version: String,
// }

// #[derive(Deserialize, Debug)]
// struct NearStatusResultBody {
//     chain_id: String,
//     rpc_addr: String,
//     sync_info: NearSyncInfo,
//     validators: Vec<NearValidatorInfo>,
//     version: NearVersionInfo,
// }

// #[derive(Deserialize, Debug)]
// struct NearStatusResult {
//     id: String,
//     jsonrpc: String,
//     result: NearStatusResultBody,
// }

// impl Default for NearStatusRequest {
//     fn default() -> Self {
//         NearStatusRequest {
//             id: "rainboltd".to_string(),
//             jsonrpc: "2.0".to_string(),
//             method: "status".to_string(),
//             params: Vec::new(),
//         }
//     }
// }