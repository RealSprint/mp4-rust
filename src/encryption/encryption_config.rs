use std::convert::TryFrom;

use crate::{frma::FrmaBox, schi::SchiBox, schm::SchmBox, sinf::SinfBox, tenc::TencBox, FourCC};

use super::initialization_vector::InitializationVector;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncryptionSchemeType {
    Cenc,
}

impl From<EncryptionSchemeType> for FourCC {
    fn from(scheme_type: EncryptionSchemeType) -> FourCC {
        match scheme_type {
            EncryptionSchemeType::Cenc => FourCC::from(*b"cenc"),
        }
    }
}

impl TryFrom<FourCC> for EncryptionSchemeType {
    type Error = String;

    fn try_from(value: FourCC) -> std::result::Result<Self, Self::Error> {
        match value {
            FourCC { value } if value == *b"cenc" => Ok(EncryptionSchemeType::Cenc),
            _ => Err("Unknown encryption scheme type".to_string()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncryptionConfig {
    scheme_type: EncryptionSchemeType,
    iv: InitializationVector,
}

impl EncryptionConfig {
    pub fn new(scheme_type: EncryptionSchemeType, iv: InitializationVector) -> Self {
        Self { scheme_type, iv }
    }

    pub fn to_sinf(&self, data_format: FourCC) -> SinfBox {
        SinfBox {
            frma: FrmaBox { data_format },
            schi: Some(SchiBox {
                tenc: TencBox::new_kid_protected(self.iv.clone()),
            }),
            schm: Some(SchmBox {
                flags: 0,
                scheme_type: self.scheme_type.clone().into(),
                scheme_uri: None,
                scheme_version: 0x10000,
                version: 0,
            }),
        }
    }
}
