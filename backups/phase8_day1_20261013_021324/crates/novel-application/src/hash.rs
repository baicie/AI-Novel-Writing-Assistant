use sha2::{Digest, Sha256};

use crate::error::AppError;

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

pub fn json_hash<T: serde::Serialize>(value: &T) -> Result<String, AppError> {
    let bytes = serde_json::to_vec(value).map_err(|_| AppError::validation("请求内容无法整理"))?;
    Ok(sha256_hex(&bytes))
}
