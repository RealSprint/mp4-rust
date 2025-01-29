use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct InitializationVector {
    pub(crate) size: u8,
    pub(crate) data: [u8; 16],
}

impl InitializationVector {
    pub fn new_64_bit(data: [u8; 8]) -> Self {
        let mut iv = [0; 16];
        iv[..8].copy_from_slice(&data);

        InitializationVector { size: 8, data: iv }
    }

    pub fn new_128_bit(data: [u8; 16]) -> Self {
        InitializationVector { size: 16, data }
    }

    pub fn new(size: Vec<u8>) -> Result<Self, InitializationVectorError> {
        if size.len() != 8 && size.len() != 16 {
            return Err(InitializationVectorError::InvalidSize(size.len()));
        }

        let mut iv = [0; 16];
        iv[..size.len()].copy_from_slice(&size);

        Ok(InitializationVector {
            size: size.len() as u8,
            data: iv,
        })
    }
}

#[derive(Error, Debug)]
pub enum InitializationVectorError {
    #[error("Invalid size: {0}")]
    InvalidSize(usize),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_64_bit() {
        let iv = InitializationVector::new_64_bit([1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(iv.size, 8);
        assert_eq!(iv.data, [1, 2, 3, 4, 5, 6, 7, 8, 0, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn test_new_128_bit() {
        let iv = InitializationVector::new_128_bit([
            1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16,
        ]);
        assert_eq!(iv.size, 16);
        assert_eq!(
            iv.data,
            [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16,]
        );
    }

    #[test]
    fn test_new_64_bit_from_vec() {
        let iv = InitializationVector::new(vec![1, 2, 3, 4, 5, 6, 7, 8]).unwrap();
        assert_eq!(iv.size, 8);
        assert_eq!(iv.data, [1, 2, 3, 4, 5, 6, 7, 8, 0, 0, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn test_new_128_bit_from_vec() {
        let iv =
            InitializationVector::new(vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16])
                .unwrap();
        assert_eq!(iv.size, 16);
        assert_eq!(
            iv.data,
            [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16,]
        );
    }
}
