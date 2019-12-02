use serde::{Serialize, Deserialize};
use near_crypto::SecretKey;

use std::{
    fs::File,
    path::Path,
    io::{Read, BufReader},
};

const DEFAULT_IP: &'static str = "http://localhost:3031";

#[derive(Serialize, Deserialize, Clone)]
pub struct RainboltdConfig {
    pub chains: Option<Vec<ChainConfig>>,
    #[serde(default)]
    pub channel_ip: String,
    #[serde(default)]
    pub recv_pay: String,
}

impl Default for RainboltdConfig {
    fn default() -> Self {
        RainboltdConfig { 
            chains: None,
            channel_ip: DEFAULT_IP.to_string(),
            recv_pay: format!("{}/maker/recvPay", DEFAULT_IP),
        }
    }
}

impl RainboltdConfig {
    pub fn ip(&self) -> &str {
        &self.channel_ip
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ChainConfig {
    pub chain_id: String,
    pub account_id: String,
    // TODO create a wrapper enum for differing SecretKey types
    pub secret_key: SecretKey,
}

pub fn load_config(path: Option<&String>) -> RainboltdConfig {
    match path {
        None => RainboltdConfig::default(),
        Some(config_path) => {
            let file = File::open(Path::new(config_path)).expect("Could not open config file");
            serde_json::from_reader(BufReader::new(file)).expect("Could not parse config file")
        }
    }
}