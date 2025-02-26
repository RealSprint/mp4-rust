use std::{convert::TryFrom, fmt::Display};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Hash)]
#[serde(try_from = "String")]
pub struct DrmKey([u8; 16]);

impl DrmKey {
    pub fn data(&self) -> &[u8; 16] {
        &self.0
    }

    pub fn new(data: [u8; 16]) -> Self {
        DrmKey(data)
    }

    pub fn from_hex(value: &str) -> Result<Self, DrmKeyError> {
        let mut key = [0; 16];
        hex::decode_to_slice(value, &mut key)?;

        Ok(DrmKey(key))
    }
}

impl Display for DrmKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        hex::encode(self.data()).fmt(f)
    }
}

impl TryFrom<&str> for DrmKey {
    type Error = DrmKeyError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::from_hex(value)
    }
}

impl TryFrom<String> for DrmKey {
    type Error = DrmKeyError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::from_hex(&value)
    }
}

impl Serialize for DrmKey {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&self)
    }
}

#[derive(Error, Debug)]
pub enum DrmKeyError {
    #[error(transparent)]
    InvalidKey(#[from] hex::FromHexError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let key = DrmKey::from_hex("0102030405060708090a0b0c0d0e0f10").unwrap();
        assert_eq!(
            key.data(),
            &[
                0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, //
                0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10, //
            ]
        );
    }

    #[test]
    fn test_invalid_key_length() {
        let result = DrmKey::from_hex("0102030405060708090a0b0c0d0e0f");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Invalid string length");
    }

    #[test]
    fn test_uneven_bytes() {
        let result = DrmKey::from_hex("1");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), "Odd number of digits");
    }

    #[test]
    fn test_invalid_key() {
        let result = DrmKey::from_hex("0102030405060708090a0b0c0d0e0fzz");
        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().to_string(),
            "Invalid character 'z' at position 30"
        );
    }
}
