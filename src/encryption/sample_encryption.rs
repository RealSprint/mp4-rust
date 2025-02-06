use serde::Serialize;

use super::initialization_vector::InitializationVector;

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct SampleEncryption {
    pub initialization_vector: InitializationVector,
    pub subsamples: Vec<SubSampleEncryption>,
}

impl SampleEncryption {
    pub fn sub_samples(&self) -> &Vec<SubSampleEncryption> {
        &self.subsamples
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct SubSampleEncryption {
    pub clear_data: u16,
    pub encrypted_data: u32,
}
