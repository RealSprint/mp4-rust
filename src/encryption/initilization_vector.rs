#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitializationVector {
    pub(crate) size: u8,
    pub(crate) data: [u8; 16],
}

// TODO: Double check if this is writter correctly - ISO/IEC 23001-7:2023(E) - 9.2
// If the Per_Sample_IV_Size field is 8, then the 128-bit IV value is made of
// InitializationVector value copied to bytes 0 to 7 and the bytes 8 to 15 are set to zero.
impl InitializationVector {
    pub fn new_64_bit(data: [u8; 8]) -> Self {
        let mut iv = [0; 16];
        iv[..8].copy_from_slice(&data);

        InitializationVector { size: 8, data: iv }
    }

    pub fn new_128_bit(data: [u8; 16]) -> Self {
        InitializationVector { size: 16, data }
    }
}
