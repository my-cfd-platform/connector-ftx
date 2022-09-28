use std::time::{SystemTime, UNIX_EPOCH};

use hmac_sha256::HMAC;

pub struct FtxAuthSettings {
    pub api_key: String,
    pub api_secret: String,
    pub subaccount: Option<String>,
}

impl FtxAuthSettings {
    pub fn generate_sign(&self, method: &str, timestamp: u128) -> String {
        let sign_payload = format!("{}{}", timestamp, method);
        let sign = HMAC::mac(sign_payload.as_bytes(), self.api_secret.as_bytes());
        let sign = hex::encode(sign);

        sign
    }

    pub fn generate_timestamp() -> u128 {
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis();

        timestamp
    }
}