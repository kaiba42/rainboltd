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
use super::model::{
    CreateOrderTx,
    CreateOrderReq,
    Coin,
    BaseRequest,
    CosmosRequest,
};
use base64;
use std::{
    fs,
    io,
    io::Write,
    process,
    process::Stdio,
};

use crate::maker::{Maker, MakerState};

const RAINBOW_NODE: &'static str = "http://localhost:1317";

// pub async fn find_merchants() -> Vec<OpenOrders> {

// }

pub async fn get_create_order_tx_to_sign(merchant: String, channel_state: ChannelState<Bls12>, channel_token: ChannelToken<Bls12>, amount: String) -> String {
    let base_req = BaseRequest {
        from: merchant.clone(),
        chain_id: "nameservice".to_string()
    };

    let order = CreateOrderReq {
        merchant,
        channel_state: base64::encode(&serde_json::to_vec(&channel_state).unwrap()),
        channel_token: base64::encode(&serde_json::to_vec(&channel_token).unwrap()),
        amount,
        // amount: Coin {
        //     denom: "boofbtc".to_string(),
        //     amount,
        // }
    };

    let msg = CosmosRequest {
        base_req,
        request: order,
    };

    // let serde_stuf = dbg!(serde_json::to_string(&msg).unwrap());

    let client = Client::new();
    let res1 = client.post(&format!("{}/nameservice/orders", RAINBOW_NODE))
        .json(&msg)
        .send()
        .await
        .expect("Could not reach cosmos node");
    // dbg!(res1.text().unwrap());
    // let res2 = res1
    //     .json()
    //     .await
    //     .expect("Could not parse cosmos response");

    // res2
    let res: String = res1.text().await.unwrap();
    res
}

// nscli tx sign unsignedTxPut.json --from jules --offline --chain-id nameservice --sequence 1 --account-number 0 > signedTxPut.json
fn sign_tx(tx: String, sequence: String, name: String) {
    let path = "unsignedTx.json".to_string();
    fs::write(&path, tx).expect("Could not write tx to disk");
    let mut out = process::Command::new("nscli tx sign")
        // .arg("tx")
        // .arg("sign")
        .arg("./unsignedTx.json")
        .arg("--from")
        .arg("jules") // should use name
        .arg("--offline")
        // .arg("-y")
        .arg("--chain-id")
        .arg("nameservice")
        .arg("--sequence")
        .arg("1") // should use sequence
        .arg("--account-number")
        .arg("0")
        .arg(">")
        .arg("./signedTx.json")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("failed to sign tx");
    // let mut stdin = out.stdin.as_mut().expect("couldn't get stdint");
    // stdin.write_all("blahblah".as_bytes()).expect("failed to write password");
    // drop(stdin);
    out.stdin.unwrap().write_all("blahblah".as_bytes()).expect("failed to write to stdin");
    // let output = out.wait_with_output().expect("failed to sign");
    // println!("{:?}", output);
        
    // io::stdin().write_all("blahblah").unwrap();
    // io::stdout().write("blahblah\n").unwrap();
    // io::stdout().write_all(&out.stdout).unwrap();
}

// nscli tx broadcast signedTxPut.json
fn broadcast_tx() {
    let out = process::Command::new("nscli tx broadcast")
        // .arg("tx")
        // .arg("broadcast")
        .arg("signedTx.json")
        .output()
        .expect("failed to broadcast tx");
    io::stdout().write_all(&out.stdout).unwrap();
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn test_send_tx() {
        let merchant = "cosmos1tsffgsxt5pvfyhkfnhep4tvl8t42gywrgn0m6m".to_string();
        // let channel_state = ChannelState::<Bls12>::new("blah".to_string(), false);//Vec::from("jlasdfjklasdflkejlafds".as_bytes());
        // let channel_token = ChannelToken {
        //     pk_c: None,
        //     pk_m: secp256k1::PublicKey::from_str("blahdfkjasldjflksfe").unwrap(),
        //     cl_pk_m: bolt::cl::PublicKey::from_secret(&cl::PublicParams<Bls12>, secret: &cl::SecretKey::generate(csprng: &mut R, l: usize)),
        //     mpk: cl::PublicParams<E>,
        //     comParams: CSMultiParams<E>,
        // }; //Vec::from("jkdfljirjowj32oqrjoi3".as_bytes());

        let maker = MakerState::init(100);
        let res = dbg!(get_create_order_tx_to_sign(merchant, maker.channel_state, maker.channel_token, "100boofbtc".to_string()).await);

        panic!()
    }

    #[tokio::test]
    async fn test_broadcast_tx() {
        let merchant = "cosmos1tsffgsxt5pvfyhkfnhep4tvl8t42gywrgn0m6m".to_string();
        let maker = MakerState::init(100);
        let unsigned = get_create_order_tx_to_sign(merchant, maker.channel_state, maker.channel_token, "100boofbtc".to_string()).await;
        sign_tx(unsigned, "3".to_string(), "jules".to_string());
        broadcast_tx();
        panic!()
    }
}