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
use serde_json::{self, json};

use futures::Future;
use futures::prelude::*;
// use std::future::Future;
use futures03::{compat::Future01CompatExt as _, FutureExt as _, TryFutureExt as _};

use crate::maker::{Maker, MakerState};
use super::model::{EscrowFillMessage, EscrowLiquidityMessage, MerchantPool};

use std::{
    fs,
    io,
    io::Write,
    process,
    process::Stdio,
};

use near_jsonrpc_client;
use near_jsonrpc_client::message::{Message, Request, Response};
use near_primitives::views::{StatusResponse, AccessKeyView, AccessKeyInfoView, QueryResponse, FinalExecutionOutcomeView, FinalExecutionStatus};
use near_primitives::views::ExecutionOutcomeView;
use near_primitives::transaction::SignedTransaction;
use near_primitives::transaction::{Action, TransferAction, FunctionCallAction};
use near_primitives::serialize::to_base64;
use near_primitives::hash::{hash, CryptoHash};
use near_crypto::{InMemorySigner, SecretKey, Signer};
use borsh::BorshSerialize;

const NEAR_NODE: &'static str = "http://localhost:3030";
const ESCROW_CONTRACT: &'static str = "escrow_test2";

// pub async fn get_status() -> NearStatusResult {
//     let client = Client::new();
//     let res = client.post(NEAR_NODE)
//         .json(&NearStatusRequest::default())
//         .send()
//         .await
//         .expect("Could not connect to near node")
//         .json::<NearStatusResult>()
//         .await
//         .expect("Could not parse near response");
//     res
// }

macro_rules! json_reqwest {
    ($req:expr => $client:ident) => {
        {
            let res = $client.post(NEAR_NODE)
                .json(&$req)
                .send()
                .await
                .map_err(|e| e.to_string())?
                .json::<Response>()
                .await
                .map_err(|e| e.to_string())?;
            serde_json::from_value(res.result.map_err(|e| e.message)?)
                .map_err(|e| e.to_string())?
        }
    };

    ($req:expr => $client:ident, $parse:ty) => {
        {
            let res = $client.post(NEAR_NODE)
                .json(&$req)
                .send()
                .await
                .map_err(|e| e.to_string())?
                .json::<Response>()
                .await
                .map_err(|e| e.to_string())?;
            serde_json::from_value::<$parse>(res.result.map_err(|e| e.message)?)
                .map_err(|e| e.to_string())?
        }
    };
}

pub async fn get_status_nearclient() -> Result<StatusResponse, String> {
    let mut jrpc_client = near_jsonrpc_client::new_client(NEAR_NODE);
    jrpc_client.status().compat().await
}

//// GENERAL NODE RPC ////
pub async fn get_account_next_nonce(client: &Client, account: String) -> Result<u64, String> {
    let access_key_query = format!("access_key/{}", account);
    match json_reqwest!(Message::request("query".to_string(), Some(json!([access_key_query, ""]))) => client) {
        // FIXME should specify which access_key in some way without doing a blind index
        QueryResponse::AccessKeyList(access_keys) => Ok(access_keys[0].access_key.nonce + 1),
        _ => Err("Received incorrect response for AccessKeyList request".to_string())
    }
}

pub async fn get_status(client: &Client) -> Result<StatusResponse, String> {
    Ok(json_reqwest!(Message::request("status".to_string(), None) => client))
}

pub async fn get_last_block_hash(client: &Client) -> Result<CryptoHash, String> {
    Ok(get_status(client).await?
        .sync_info
        .latest_block_hash)
}

pub async fn broadcast_tx(client: &Client, signed_tx: &mut SignedTransaction) -> Result<FinalExecutionOutcomeView, String> {
    signed_tx.init();
    let tx = signed_tx.try_to_vec().map_err(|e| e.to_string())?;
    Ok(json_reqwest!(Message::request("broadcast_tx_commit".to_string(), Some(json!([to_base64(&tx)]))) => client))
}

//// ESCROW SPECIFIC RPC ////
pub async fn escrow_liquidity(client: &Client, signer: &InMemorySigner, maker_state: &MakerState, amount: u128) -> Result<FinalExecutionOutcomeView, String> {
    // let amount = maker_state.initial_margin;
    let account = &signer.account_id;

    let nonce = get_account_next_nonce(&client, account.clone()).await?;
    let block_hash = get_last_block_hash(&client).await?;

    let args = serde_json::to_vec(&EscrowLiquidityMessage {
        merchant_bls: maker_state.merchant_state.keys.iter().nth(0).unwrap().1.wpk, // FIXME unsure if this is the right key
        channel_state: maker_state.channel_state.clone(),
        channel_token: maker_state.channel_token.clone(),
    }).unwrap();

    let mut signed_tx = SignedTransaction::from_actions(
        nonce,
        "rainbolt_dev".to_string(),
        ESCROW_CONTRACT.to_string(),
        signer,
        vec![Action::FunctionCall(FunctionCallAction {
            method_name: "escrow_liquidity".to_string(),
            args,
            gas: 10000000,
            deposit: amount,
        })],
        block_hash
    );

    broadcast_tx(client, &mut signed_tx).await
}

pub async fn escrow_fill(client: &Client, signer: &InMemorySigner, amount: u128) {

}

pub async fn show_liquidity(client: &Client) -> Result<Vec<MerchantPool>, String> {
    let query = format!("call/{}/show_liquidity", ESCROW_CONTRACT);
    match json_reqwest!(Message::request("query".to_string(), Some(json!([query, ""]))) => client) {
        // FIXME should specify which access_key in some way without doing a blind index
        QueryResponse::CallResult(result) => Ok(serde_json::from_slice(&result.result).map_err(|e| e.to_string())?),
        _ => Err("Received incorrect response for AccessKeyList request".to_string())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    
    use std::sync::mpsc;
    use std::path::Path;

    use actix::{Actor, System};
    use near_network::test_utils::{wait_or_panic, WaitOrTimeout};

    use std::sync::{Arc, Mutex};
    // #[tokio::test]
    // async fn test_connection_to_node() {
    //     // assert_eq!(get_status().await, "Hmm".to_string());
    //     let res: NearStatusResult = get_status().await;
    //     println!("{:?}", res);
    //     panic!(res)
    // }

    #[tokio::test]
    // #[test]
    async fn test_connection_to_node_jrpcclient() {
        let res = get_status_nearclient().await;
        println!("{:?}", res);

        panic!()
    }

    #[tokio::test]
    // #[test]
    async fn sign_tx_reqwest_client() {
        let client = Client::new();
        
        let account = "rainbolt_dev".to_string();
        let nonce = get_account_next_nonce(&client, account.clone()).await.expect("Could not get nonce");
        let block_hash = get_last_block_hash(&client).await.expect("Could not get block_hash");
        // let (nonce, hash) = &mut nonce_f.join(block_hash_f).await; 
        //get_account_next_nonce(&client, account.clone()).join(get_last_block_hash(&client)).await;

        let signer = InMemorySigner::from_file(&Path::new("/Users/julian/SFBW_WORKSHOP/rainbolt_near_chain/neardev/default/rainbolt_dev.json"));
        let args = serde_json::to_vec(&EscrowFillMessage {
            merchant: "merchant_b".to_string(),
            customer_bls: "blahblah333".to_string(),
            wallet_commit: vec![],
        }).unwrap();
        let mut signed_tx = SignedTransaction::from_actions(
            nonce,
            "rainbolt_dev".to_string(),
            ESCROW_CONTRACT.to_string(),
            &signer,
            vec![Action::FunctionCall(FunctionCallAction {
                method_name: "escrow_fill".to_string(),
                args,
                gas: 10000000,
                deposit: 20,
            })],
            block_hash
        );
        
        let escrow_outcome = broadcast_tx(&client, &mut signed_tx).await.expect("Could not send tx");
        println!("{:?}", escrow_outcome);
        assert_eq!(escrow_outcome.status, FinalExecutionStatus::SuccessValue(base64::encode("\"Success\"")));
    }

    #[tokio::test]
    //#[test]
    async fn sign_tx_jrpc_client() {
        let nonce_arc = Arc::new(Mutex::new(None));
        
        System::run(move || {
            let mut jrpc_client = near_jsonrpc_client::new_client(NEAR_NODE);
            let mut view_client = near_jsonrpc_client::new_client(NEAR_NODE);
            let nonce_arc2 = nonce_arc.clone();

            actix::spawn(view_client.query("access_key/rainbolt_dev".to_string(), "".to_string()).then(move |res| {
                *nonce_arc2.lock().unwrap() = Some(match res.unwrap() {
                    QueryResponse::AccessKeyList(access_keys) => access_keys[0].access_key.nonce + 1,
                    _ => panic!("did not get access key list")
                });
                view_client.status()
            }).then(move |res| {
                let nonce = nonce_arc.clone();
                let nonce_val = nonce.lock().unwrap().unwrap();

                let block_hash = res.unwrap().sync_info.latest_block_hash;
                let signer = InMemorySigner::from_file(&Path::new("/Users/julian/SFBW_WORKSHOP/rainbolt_near_chain/neardev/default/rainbolt_dev.json"));
                println!("{}", signer.public_key());
                // let args = Vec::from(r#"{"merchant": "merchant_b", "customer_bls": "blah", "wallet_commit": []}"#.as_bytes());
                let args = serde_json::to_vec(&EscrowFillMessage {
                    merchant: "merchant_b".to_string(),
                    customer_bls: "blahblah".to_string(),
                    wallet_commit: vec![],
                }).unwrap();
                let mut signed_tx = SignedTransaction::from_actions(
                    nonce_val,
                    "rainbolt_dev".to_string(),
                    ESCROW_CONTRACT.to_string(),
                    &signer,
                    vec![Action::FunctionCall(FunctionCallAction {
                        method_name: "escrow_fill".to_string(),
                        args,
                        gas: 10000000,
                        deposit: 10,
                    })],
                    block_hash
                );
                signed_tx.init();

                let tx = signed_tx.try_to_vec().unwrap();

                jrpc_client.broadcast_tx_commit(to_base64(&tx)).then(|res| {
                    println!("{:?}", res.unwrap());
                    panic!();
                    Ok(())
                })
            }))
            // let res = jrpc_client.broadcast_tx_commit(to_base64(&tx)).compat().await;
            // println!("{:?}", res.unwrap());
        })
        .unwrap()
    }
}