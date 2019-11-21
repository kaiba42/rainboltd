use reqwest::Client;
use bolt::{
    channels::{
        ChannelState,
        ChannelToken,
    },
    cl,
};
use secp256k1;
use pairing::bls12_381::Bls12;
use serde_json::json;

use crate::maker::{Maker, MakerState};

use std::{
    fs,
    io,
    io::Write,
    process,
    process::Stdio,
};

use near_jsonrpc_client;

const NEAR_NODE: &'static str = "http://localhost:3030";

use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct NearStatusRequest {
    id: String,
    jsonrpc: String,
    method: String,
    params: Vec<u8>,
}

#[derive(Deserialize, Debug)]
struct NearSyncInfo {
    latest_block_hash: String,
    latest_block_height: u64,
    latest_block_time: String,
    latest_state_root: String,
    syncing: bool,
}

#[derive(Deserialize, Debug)]
struct NearValidatorInfo {
    account_id: String,
    is_slashed: bool,
}

#[derive(Deserialize, Debug)]
struct NearVersionInfo {
    build: String,
    version: String,
}

#[derive(Deserialize, Debug)]
struct NearStatusResultBody {
    chain_id: String,
    rpc_addr: String,
    sync_info: NearSyncInfo,
    validators: Vec<NearValidatorInfo>,
    version: NearVersionInfo,
}

#[derive(Deserialize, Debug)]
struct NearStatusResult {
    id: String,
    jsonrpc: String,
    result: NearStatusResultBody,
}

impl Default for NearStatusRequest {
    fn default() -> Self {
        NearStatusRequest {
            id: "rainboltd".to_string(),
            jsonrpc: "2.0".to_string(),
            method: "status".to_string(),
            params: Vec::new(),
        }
    }
}

// TODO Could use a macro for base line json2.0 rpc messages

pub async fn get_status() -> NearStatusResult {

    let client = Client::new();
    let res = client.post(NEAR_NODE)
        .json(&NearStatusRequest::default())
        .send()
        .await
        .expect("Could not connect to near node")
        .json::<NearStatusResult>()
        .await
        .expect("Could not parse near response");
    res
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn test_connection_to_node() {
        // assert_eq!(get_status().await, "Hmm".to_string());
        let res: NearStatusResult = get_status().await;
        println!("{:?}", res);
        panic!(res)
    }
}