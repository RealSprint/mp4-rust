use serde::Serialize;

use super::initialization_vector::InitializationVector;

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct SampleEncryption {
    pub initialization_vector: InitializationVector,
    pub sub_samples: Vec<SubSampleEncryption>,
}

impl SampleEncryption {
    pub fn sub_samples(&self) -> &Vec<SubSampleEncryption> {
        &self.sub_samples
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct SubSampleEncryption {
    pub clear_data: u16,
    pub encrypted_data: u32,
}
