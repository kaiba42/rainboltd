use crate::maker::{Maker, MakerState};
use crate::taker::{Taker, TakerState};
use crate::chain_clients::{ChainClient, MerchantPool};
use super::model::{
    EscrowFillMessage,
    EscrowLiquidityMessage,
    NearEscrowAccount,
    NearMerchantPool
};

use near_jsonrpc_client::message::{Message, Request, Response};
use near_primitives::{
    serialize::to_base64,
    hash::{hash, CryptoHash},
    transaction::{
        Action,
        TransferAction,
        FunctionCallAction,
        SignedTransaction
    },
    views::{
        StatusResponse,
        AccessKeyView,
        AccessKeyInfoView,
        QueryResponse,
        FinalExecutionOutcomeView,
        FinalExecutionStatus,
        ExecutionOutcomeView
    },
};
use near_crypto::{InMemorySigner, SecretKey, Signer};
use borsh::BorshSerialize;

use reqwest::Client;
use bolt::channels::{ChannelState, ChannelToken};
use bolt::ped92::Commitment;
use pairing::bls12_381::Bls12;
use secp256k1;

use serde_json::{self, json};

use async_trait::async_trait;
use futures::try_join;

use std::{path::Path};
use std::str::FromStr; // Remove Me

const NEAR_NODE: &'static str = "http://localhost:3030";
const ESCROW_CONTRACT: &'static str = "escrow_test2";

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
}

pub struct NearChainClient {
    client: Client,
    signer: InMemorySigner,
}

impl NearChainClient {
    pub fn from_secret_key(client: Client, account_id: String, secret_key: SecretKey) -> Self {
        NearChainClient { 
            client,
            signer: InMemorySigner::from_secret_key(account_id, secret_key) 
        }
    }

    pub fn from_file(client: Client, key_file: &Path) -> Self {
        NearChainClient {
            client,
            signer: InMemorySigner::from_file(key_file)
        }
    }
}

#[async_trait]
impl ChainClient for NearChainClient {
    async fn sign_and_send_liquidity_msg(&self, maker_state: &MakerState, amount: u128) -> Result<String, String> {
        match escrow_liquidity(&self.client, &self.signer, maker_state, amount).await?.status {
            FinalExecutionStatus::SuccessValue(success) => Ok(success),
            _ => unimplemented!()
        }
    }

    async fn sign_and_send_fill_msg(&self, taker_state: &TakerState, merchant: String, amount: u128) -> Result<String, String> {
        match escrow_fill(&self.client, &self.signer, taker_state, merchant, amount).await?.status {
            FinalExecutionStatus::SuccessValue(success) => Ok(success),
            _ => unimplemented!()
        }
    }

    async fn show_liquidity(&self) -> Result<Vec<MerchantPool>, String> {
        Ok(show_liquidity(&self.client)
            .await?
            .into_iter()
            .map(|pool| MerchantPool::from(pool))
            .collect())
    }
}

//// GENERAL NODE RPC ////
async fn get_account_next_nonce(client: &Client, account: String) -> Result<u64, String> {
    let access_key_query = format!("access_key/{}", account);
    match json_reqwest!(Message::request("query".to_string(), Some(json!([access_key_query, ""]))) => client) {
        // FIXME should specify which access_key in some way without doing a blind index
        QueryResponse::AccessKeyList(access_keys) => Ok(access_keys[0].access_key.nonce + 1),
        _ => Err("Received incorrect response for AccessKeyList request".to_string())
    }
}

async fn get_status(client: &Client) -> Result<StatusResponse, String> {
    Ok(json_reqwest!(Message::request("status".to_string(), None) => client))
}

async fn get_last_block_hash(client: &Client) -> Result<CryptoHash, String> {
    Ok(get_status(client).await?
        .sync_info
        .latest_block_hash)
}

async fn broadcast_tx(client: &Client, signed_tx: &mut SignedTransaction) -> Result<FinalExecutionOutcomeView, String> {
    signed_tx.init();
    let tx = signed_tx.try_to_vec().map_err(|e| e.to_string())?;
    Ok(json_reqwest!(Message::request("broadcast_tx_commit".to_string(), Some(json!([to_base64(&tx)]))) => client))
}

//// ESCROW SPECIFIC RPC ////
async fn escrow_liquidity(client: &Client, signer: &InMemorySigner, maker_state: &MakerState, amount: u128) -> Result<FinalExecutionOutcomeView, String> {
    // let amount = maker_state.initial_margin;
    let account = signer.account_id.clone();
    let (nonce, block_hash) = try_join!(get_account_next_nonce(&client, account.clone()), get_last_block_hash(&client))?;

    let args = serde_json::to_vec(&EscrowLiquidityMessage {
        // FIXME unsure if this is the right key
        merchant_bls: maker_state.merchant_state.pk, 
        // TODO Should probably be zipped or otherwise compressed to save space (and therefore gas) on chain
        channel_state: base64::encode(&serde_json::to_vec(&maker_state.channel_state).unwrap()), //maker_state.channel_state.clone(),
        channel_token: base64::encode(&serde_json::to_vec(&maker_state.channel_token).unwrap()), //maker_state.channel_token.clone(),
    }).unwrap();

    let mut signed_tx = SignedTransaction::from_actions(
        nonce,
        account,
        ESCROW_CONTRACT.to_string(),
        signer,
        vec![Action::FunctionCall(FunctionCallAction {
            method_name: "escrow_liquidity".to_string(),
            args,
            gas: 1000000000,
            deposit: amount,
        })],
        block_hash
    );

    broadcast_tx(client, &mut signed_tx).await
}

async fn escrow_fill(client: &Client, signer: &InMemorySigner, taker_state: &TakerState, merchant: String, amount: u128) -> Result<FinalExecutionOutcomeView, String> {
    let account = signer.account_id.clone();
    let (nonce, block_hash) = futures::try_join!(get_account_next_nonce(&client, account.clone()), get_last_block_hash(&client))?;

    let message = EscrowFillMessage {
        merchant,
        // FIXME is this the correct key?
        customer_bls: taker_state.customer_state.wpk, // Wallet Public Key
        // FIXME should compress and not do this Vec dance
        wallet_commit: Vec::from(base64::encode(&serde_json::to_vec(&taker_state.root_commitment).unwrap()).as_bytes()),
    };

    let mut signed_tx = SignedTransaction::from_actions(
        nonce,
        account,
        ESCROW_CONTRACT.to_string(),
        signer,
        vec![Action::FunctionCall(FunctionCallAction {
            method_name: "escrow_fill".to_string(),
            args: serde_json::to_vec(&message).unwrap(),
            gas: 1000000000,
            deposit: amount,
        })],
        block_hash
    );

    broadcast_tx(client, &mut signed_tx).await
}

async fn show_liquidity(client: &Client) -> Result<Vec<NearMerchantPool>, String> {
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
    use std::str::FromStr;

    #[tokio::test]
    //#[test]
    async fn send_liquidity_message() {
        let amount = 50;
        let maker = MakerState::init(amount);
        let client = Client::new();
        let signer = InMemorySigner::from_file(&Path::new("/Users/julian/SFBW_WORKSHOP/rainbolt_near_chain/neardev/default/rainbolt_dev.json"));

        let res = escrow_liquidity(&client, &signer, &maker, amount as u128).await;
        println!("RES: {:?}", res);
        assert!(res.is_ok());
        assert_eq!(res.unwrap().status, FinalExecutionStatus::SuccessValue(base64::encode("\"Success\"")));
    }

    #[tokio::test]
    //#[test]
    async fn deserialize_liquidity() {
        let client = Client::new();
        let liquidity = show_liquidity(&client).await.unwrap();
        let pool2 = &liquidity[2];
        let channel_state: ChannelState<Bls12> = serde_json::from_slice(&base64::decode(&pool2.channel_state).unwrap()).unwrap();
        let channel_token: ChannelToken<Bls12> = serde_json::from_slice(&base64::decode(&pool2.channel_token).unwrap()).unwrap();
        // R: i32,
        // tx_fee: i64,
        // pub cp: Option<ChannelParams<E>>,
        // pub name: String,
        // pub pay_init: bool,
        // pub channel_established: bool,
        // pub third_party: bool,
        println!("{} {} {} {}", channel_state.name, channel_state.pay_init, channel_state.channel_established, channel_state.third_party);
    }

    #[tokio::test]
    //#[test]
    async fn send_fill_message() {
        let client = Client::new();
        let liquidity = show_liquidity(&client).await.unwrap();
        let MerchantPool { channel_state, channel_token, available: amount, ..} = MerchantPool::from(liquidity.into_iter().nth(2).unwrap());

        let signer = InMemorySigner::from_file(&Path::new("/Users/julian/SFBW_WORKSHOP/rainbolt_near_chain/neardev/default/rainbolt_dev.json"));

        let taker = TakerState::init(amount as i64, amount as i64, channel_state, channel_token);
        
        let res = escrow_fill(&client, &signer, &taker, "rainbolt_dev".to_string(), amount).await;
        println!("RES: {:?}", res);
    }
}