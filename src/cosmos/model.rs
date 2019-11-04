use serde::{Serialize, Deserialize};
use bolt::{
    channels::{
        ChannelState,
        ChannelToken,
    }
};

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FillOrder {
    customer: Vec<u8>,
    wallet_commit: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug)]
// #[serde(rename_all = "camelCase")]
pub struct CosmosRequest {
    pub base_req: BaseRequest,
    #[serde(flatten)]
    pub request: CreateOrderReq
}

#[derive(Serialize, Deserialize, Debug)]
// #[serde(rename_all = "camelCase")]
pub struct BaseRequest {
    pub from: String,
    pub chain_id: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Coin {
    pub denom: String,

    pub amount: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CreateOrderReq {
    pub merchant: String,
    pub channel_state: String,
    pub channel_token: String,
    pub amount: String,//Coin,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CreateOrderRes {
    pub merchant: String,
    pub channel_state: String,
    pub channel_token: String,
    pub amount: Vec<Coin>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CreateOrderTx {
    r#type: String,
    value: CreateOrderTxValue,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CreateOrderTxValue {
    msg: Vec<CreateOrderTxMessage>,
    fee: TxFee,
    signatures: Option<String>, // Figure out bytes
    memo: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct CreateOrderTxMessage {
    r#type: String,
    value: CreateOrderRes,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TxFee {
    // #[serde(flatten)]
    amount: Vec<Coin>,
    gas: String
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct OpenOrder {
    merchant: String,
    channel_state: String,
    channel_token: String,
    
}



