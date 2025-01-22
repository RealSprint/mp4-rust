use std::convert::TryFrom;

use crate::{
    frma::FrmaBox, pssh::PsshBox, schi::SchiBox, schm::SchmBox, sinf::SinfBox, tenc::TencBox,
    FourCC,
};

#[derive(Debug, Clone, PartialEq, Eq)]
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
}

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
    system_id: [u8; 16],
}

impl EncryptionConfig {
    pub fn new(
        scheme_type: EncryptionSchemeType,
        iv: InitializationVector,
        system_id: [u8; 16],
    ) -> Self {
        Self {
            scheme_type,
            iv,
            system_id,
        }
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

    pub fn to_pssh(&self) -> PsshBox {
        // TODO: Support multiple KIDs ?
        PsshBox::with_kid(self.system_id, vec![self.iv.data], Vec::new())
    }

    pub fn from_pssh(pssh: &PsshBox) -> Option<Self> {
        // TODO: What about data?
        let kid = pssh.get_kid();
        if kid.is_empty() {
            return None;
        }

        // TODO: Support multiple KIDs ?
        Some(Self {
            scheme_type: EncryptionSchemeType::Cenc,
            iv: InitializationVector::new_128_bit(kid[0]),
            system_id: *pssh.get_system_id(),
        })
    }
}
