use bolt::{
    ped92::{Commitment, CommitmentProof},
    cl::Signature,
    bidirectional::{
        Payment,
        RevokeToken
    },
};
use serde::{Serialize, Deserialize};
use secp256k1;
use pairing::bls12_381::Bls12;

#[derive(Serialize, Deserialize)]
pub struct OpenChannelRequest {
    pub customer_public_key: secp256k1::PublicKey,
    pub root_commitment: Commitment<Bls12>,
    pub root_commitment_proof: CommitmentProof<Bls12>,
    pub margin: i64,
    pub order_size: i64,
}

#[derive(Serialize, Deserialize)]
pub struct OpenChannelResponse {
    pub close_token: Signature<Bls12>,
    pub pay_token: Signature<Bls12>
}

#[derive(Serialize, Deserialize)]
pub struct PaymentRequest {
    pub payment_proof: Payment<Bls12>
}

#[derive(Serialize, Deserialize)]
pub struct PaymentResponse {
    pub close_token: Signature<Bls12>
}

#[derive(Serialize, Deserialize)]
pub struct GeneratePaymentTokenRequest {
    pub revoke_token: RevokeToken
}

#[derive(Serialize, Deserialize)]
pub struct GeneratePaymentTokenResponse {
    pub payment_token: Signature<Bls12>
} 

#[derive(Serialize, Deserialize)]
pub struct OpenMarketState {
    pub last_index_price: f64,
}

#[derive(Serialize, Deserialize)]
pub struct OrderRequest {
    pub initial_margin: i64, 
    pub order_size: i64, 
    pub maker_order_id: String,
}