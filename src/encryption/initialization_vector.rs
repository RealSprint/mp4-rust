use std::fmt::Display;

use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct InitializationVector {
    value: u128,
    size: u8,
}
// ISO 23001-7:2023 9.2 Initialization Vector
impl InitializationVector {
    pub fn new_64_bit(data: [u8; 8]) -> Self {
        Self {
            value: u64::from_be_bytes(data) as u128,
            size: 8,
        }
    }

    pub fn new_128_bit(data: [u8; 16]) -> Self {
        Self {
            value: u128::from_be_bytes(data),
            size: 16,
        }
    }

    pub fn new(data: Vec<u8>) -> Result<Self, InitializationVectorError> {
        match data.len() {
            8 => {
                let mut iv = [0; 8];
                iv[..8].copy_from_slice(&data);
                Ok(Self::new_64_bit(iv))
            }
            16 => {
                let mut iv = [0; 16];
                iv[..16].copy_from_slice(&data);
                Ok(Self::new_128_bit(iv))
            }
            _ => Err(InitializationVectorError::InvalidSize(data.len() as u8)),
        }
    }

    pub fn new_from_size(size: u8, data: [u8; 16]) -> Result<Self, InitializationVectorError> {
        match size {
            8 => {
                let mut iv = [0; 8];
                iv[..8].copy_from_slice(&data[0..8]);

                Ok(Self::new_64_bit(iv))
            }
            16 => Ok(Self::new_128_bit(data)),
            _ => Err(InitializationVectorError::InvalidSize(size)),
        }
    }

    /// Increment the initialization vector by the number of bytes processed,
    /// to increase entropy when encrypting.
    pub fn increment(&mut self, bytes: usize) {
        // There isn't anything in the specification on how to handle overflows for 16 byte IVs.

        // For 8 byte IVs, the IV should wrap around to 0, which isn't technically what we do here,
        // but as we only return the first 8 bytes, it doesn't matter.

        self.value = self.value.wrapping_add((bytes / 16) as u128);
    }

    pub fn data(&self) -> [u8; 16] {
        match self.size {
            8 => u128::to_be_bytes(self.value << 64),
            16 => u128::to_be_bytes(self.value),
            _ => unreachable!(),
        }
    }

    pub fn size(&self) -> u8 {
        self.size
    }
}

impl Display for InitializationVector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for b in self.data() {
            write!(f, "{:02x}", b)?;
        }

        Ok(())
    }
}

#[derive(Error, Debug)]
pub enum InitializationVectorError {
    #[error("Invalid size: {0}")]
    InvalidSize(u8),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_64_bit() {
        let iv = InitializationVector::new_64_bit([0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]);
        assert_eq!(
            iv.data(),
            [
                0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, //
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
            ]
        );
    }

    #[test]
    fn test_new_128_bit() {
        let iv = InitializationVector::new_128_bit([
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, //
            0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10, //
        ]);
        assert_eq!(
            iv.data(),
            [
                0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, //
                0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10, //
            ]
        );
    }

    #[test]
    fn test_new_64_bit_from_vec() {
        let iv = InitializationVector::new(vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08])
            .unwrap();
        assert_eq!(
            iv.data(),
            [
                0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, //
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
            ]
        );
    }

    #[test]
    fn test_new_128_bit_from_vec() {
        let iv = InitializationVector::new(vec![
            0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, //
            0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10, //
        ])
        .unwrap();
        assert_eq!(
            iv.data(),
            [
                0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, //
                0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10, //
            ]
        );
    }

    #[test]
    fn test_increment_128() {
        let mut iv = InitializationVector::new(vec![
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
        ])
        .unwrap();

        assert_eq!(
            iv.data(),
            [
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
            ]
        );

        iv.increment(32);
        assert_eq!(
            iv.data(),
            [
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, //
            ]
        );
    }

    #[test]
    fn test_increment_64() {
        let mut iv = InitializationVector::new(vec![
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
        ])
        .unwrap();

        assert_eq!(
            iv.data(),
            [
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
            ]
        );

        iv.increment(32);
        assert_eq!(
            iv.data(),
            [
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, //
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
            ]
        );
    }

    #[test]
    fn test_wrapping() {
        let mut iv = InitializationVector::new_64_bit([
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, //
        ]);

        assert_eq!(
            iv.data(),
            [
                0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, //
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
            ]
        );

        iv.increment(16);
        assert_eq!(
            iv.data(),
            [
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
            ]
        );
    }
}
