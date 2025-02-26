use std::io::{Cursor, Read, Seek, Write};

use base64::{prelude::BASE64_STANDARD, Engine};
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use serde::{de, Serialize};

use crate::encryption::drm_key::DrmKey;

use super::{
    box_start, skip_bytes_to, write_box_header_ext, BoxHeader, BoxType, Error, Mp4Box, Mp4Context,
    ReadBox, Result, WriteBox, HEADER_EXT_SIZE, HEADER_SIZE,
};

// ISO 23001-7:2023 - 8.1 Protection System Specific Header Box
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PsshBox {
    version: u8,
    flags: u32,

    system_id: DrmKey,

    kid_count: Option<u32>,
    kid: Vec<DrmKey>,

    data_size: u32,
    data: Vec<u8>,
}

impl PsshBox {
    pub fn from_base64(base64: &str) -> Result<Self> {
        let data = BASE64_STANDARD.decode(base64.as_bytes())?;

        let mut reader = Cursor::new(&data);
        let header = BoxHeader::read(&mut reader)?;

        PsshBox::read_box(&mut reader, header.size, &mut Mp4Context::default())
    }

    pub fn new_clearkey() -> Self {
        Self::new(
            DrmKey::from_hex("1077efecc0b24d02ace33c1e52e2fb4b").expect("always valid"),
            vec![],
        )
    }

    pub fn new(system_id: DrmKey, data: Vec<u8>) -> Self {
        PsshBox {
            version: 0,
            flags: 0,

            system_id,

            kid_count: None,
            kid: Vec::new(),

            data_size: data.len() as u32,
            data,
        }
    }

    pub fn with_kid(system_id: DrmKey, kid: Vec<DrmKey>, data: Vec<u8>) -> Self {
        PsshBox {
            version: 1,
            flags: 0,

            system_id,

            kid_count: Some(kid.len() as u32),
            kid,

            data_size: data.len() as u32,
            data,
        }
    }

    pub fn get_type(&self) -> BoxType {
        BoxType::PsshBox
    }

    pub fn get_size(&self) -> u64 {
        let kid_size = if self.version > 0 {
            let kid_count = self.kid_count.expect("always set for version > 0") as u64;
            4 + 16 * kid_count
        } else {
            0
        };

        let data_size = 4 + self.data_size as u64;

        HEADER_SIZE + HEADER_EXT_SIZE + 16 + kid_size + data_size
    }

    pub fn to_base64(&self) -> String {
        let mut buf = Vec::new();
        self.write_box(&mut buf).unwrap();

        BASE64_STANDARD.encode(&buf)
    }
}

impl Mp4Box for PsshBox {
    fn box_type(&self) -> BoxType {
        self.get_type()
    }

    fn box_size(&self) -> u64 {
        self.get_size()
    }

    fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string(&self).unwrap_or_default())
    }

    fn summary(&self) -> Result<String> {
        let s = format!(
            "system_id={:?}, kid_count={:?}, kid={:?}, data_size={}, data={:?}",
            self.system_id, self.kid_count, self.kid, self.data_size, self.data
        );
        Ok(s)
    }
}

impl<R: Read + Seek> ReadBox<&mut R> for PsshBox {
    fn read_box(reader: &mut R, size: u64, _context: &mut Mp4Context) -> Result<Self> {
        let start = box_start(reader)?;

        let (version, flags) = super::read_box_header_ext(reader)?;

        let mut system_id = [0; 16];
        reader.read_exact(&mut system_id)?;

        let (kid_count, kid) = if version > 0 {
            let kid_count = reader.read_u32::<BigEndian>()?;
            let mut kid = vec![[0; 16]; kid_count as usize];
            for i in 0..kid_count {
                reader.read_exact(&mut kid[i as usize])?;
            }
            (
                Some(kid_count),
                kid.iter().map(|x| DrmKey::new(*x)).collect(),
            )
        } else {
            (None, Vec::new())
        };

        let data_size = reader.read_u32::<BigEndian>()?;
        let mut data = vec![0; data_size as usize];
        reader.read_exact(&mut data)?;

        skip_bytes_to(reader, start + size)?;

        Ok(PsshBox {
            version,
            flags,
            system_id: DrmKey::new(system_id),
            kid_count,
            kid,
            data_size,
            data,
        })
    }
}

impl<W: Write> WriteBox<&mut W> for PsshBox {
    fn write_box(&self, writer: &mut W) -> Result<u64> {
        let size = self.box_size();

        BoxHeader::new(self.box_type(), size).write(writer)?;

        write_box_header_ext(writer, self.version, self.flags)?;

        writer.write_all(self.system_id.data())?;

        if self.version > 0 {
            let kid_count = match self.kid_count {
                Some(kid_count) => kid_count,
                None => return Err(Error::InvalidData("kid_count is required for version > 0")),
            };

            writer.write_u32::<BigEndian>(kid_count)?;

            for i in 0..kid_count {
                writer.write_all(self.kid[i as usize].data())?;
            }
        }

        writer.write_u32::<BigEndian>(self.data_size)?;
        writer.write_all(&self.data)?;

        Ok(size)
    }
}

impl Serialize for PsshBox {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_str(&self.to_base64())
    }
}

impl<'de> de::Deserialize<'de> for PsshBox {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::from_base64(&s).map_err(|err| <D::Error as serde::de::Error>::custom(err.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mp4box::BoxHeader;
    use std::io::Cursor;

    #[test]
    fn test_pssh() {
        let system_id = DrmKey::new([
            0x10, 0x77, 0xef, 0xec, 0xc0, 0xb2, 0x4d, 0x02, //
            0xac, 0xe3, 0x3c, 0x1e, 0x52, 0xe2, 0xfb, 0x4b,
        ]);

        let data = vec![
            0x6d, 0x76, 0xf2, 0x5c, 0xb1, 0x7f, 0x5e, 0x16, //
        ];

        let src_box = PsshBox::new(system_id, data);

        let mut buf = Vec::new();
        src_box.write_box(&mut buf).unwrap();
        assert_eq!(buf.len(), src_box.box_size() as usize);

        let expected: Vec<u8> = vec![
            0x00, 0x00, 0x00, 0x28, b'p', b's', b's', b'h', // header
            0x00, 0x00, 0x00, 0x00, // header ext
            0x10, 0x77, 0xef, 0xec, 0xc0, 0xb2, 0x4d, 0x02, // system id
            0xac, 0xe3, 0x3c, 0x1e, 0x52, 0xe2, 0xfb, 0x4b, // system id
            0x00, 0x00, 0x00, 0x08, // data size
            0x6d, 0x76, 0xf2, 0x5c, 0xb1, 0x7f, 0x5e, 0x16, // data
        ];
        assert_eq!(buf, expected);

        let mut reader = Cursor::new(&buf);
        let header = BoxHeader::read(&mut reader).unwrap();
        assert_eq!(header.name, BoxType::PsshBox);
        assert_eq!(src_box.box_size(), header.size);

        let dst_box =
            PsshBox::read_box(&mut reader, header.size, &mut Mp4Context::default()).unwrap();
        assert_eq!(src_box, dst_box);
    }

    #[test]
    fn test_pssh_with_kid() {
        let kid = vec![DrmKey::new([
            0x6d, 0x76, 0xf2, 0x5c, 0xb1, 0x7f, 0x5e, 0x16, //
            0xb8, 0xea, 0xef, 0x6b, 0xbf, 0x58, 0x2d, 0x8e,
        ])];

        let system_id = DrmKey::new([
            0x10, 0x77, 0xef, 0xec, 0xc0, 0xb2, 0x4d, 0x02, //
            0xac, 0xe3, 0x3c, 0x1e, 0x52, 0xe2, 0xfb, 0x4b,
        ]);

        let src_box = PsshBox::with_kid(system_id, kid, Vec::new());

        let mut buf = Vec::new();
        src_box.write_box(&mut buf).unwrap();
        assert_eq!(buf.len(), src_box.box_size() as usize);

        let expected: Vec<u8> = vec![
            0x00, 0x00, 0x00, 0x34, b'p', b's', b's', b'h', //
            0x01, 0x00, 0x00, 0x00, 0x10, 0x77, 0xef, 0xec, //
            0xc0, 0xb2, 0x4d, 0x02, 0xac, 0xe3, 0x3c, 0x1e, //
            0x52, 0xe2, 0xfb, 0x4b, 0x00, 0x00, 0x00, 0x01, //
            0x6d, 0x76, 0xf2, 0x5c, 0xb1, 0x7f, 0x5e, 0x16, //
            0xb8, 0xea, 0xef, 0x6b, 0xbf, 0x58, 0x2d, 0x8e, //
            0x00, 0x00, 0x00, 0x00,
        ];
        assert_eq!(buf, expected);

        let mut reader = Cursor::new(&buf);
        let header = BoxHeader::read(&mut reader).unwrap();
        assert_eq!(header.name, BoxType::PsshBox);
        assert_eq!(src_box.box_size(), header.size);

        let dst_box =
            PsshBox::read_box(&mut reader, header.size, &mut Mp4Context::default()).unwrap();
        assert_eq!(src_box, dst_box);
    }

    #[test]
    fn widevine() {
        let base64 =
            "AAAAP3Bzc2gAAAAA7e+LqXnWSs6jyCfc1R0h7QAAAB8SEKIFDqR9T0FWsI36mib6TasaBWV6ZHJtSPPGiZsG";

        let pssh = PsshBox::from_base64(base64).unwrap();

        assert_eq!(
            pssh.system_id.data(),
            &[
                0xed, 0xef, 0x8b, 0xa9, 0x79, 0xd6, 0x4a, 0xce, //
                0xa3, 0xc8, 0x27, 0xdc, 0xd5, 0x1d, 0x21, 0xed //
            ]
        );

        assert_eq!(pssh.data_size, 31)
    }

    #[test]
    fn playready() {
        let base64 = "AAACvnBzc2gAAAAAmgTweZhAQoarkuZb4IhflQAAAp6eAgAAAQABAJQCPABXAFIATQBIAEUAQQBEAEUAUgAgAHgAbQBsAG4AcwA9ACIAaAB0AHQAcAA6AC8ALwBzAGMAaABlAG0AYQBzAC4AbQBpAGMAcgBvAHMAbwBmAHQALgBjAG8AbQAvAEQAUgBNAC8AMgAwADAANwAvADAAMwAvAFAAbABhAHkAUgBlAGEAZAB5AEgAZQBhAGQAZQByACIAIAB2AGUAcgBzAGkAbwBuAD0AIgA0AC4AMwAuADAALgAwACIAPgA8AEQAQQBUAEEAPgA8AFAAUgBPAFQARQBDAFQASQBOAEYATwA+ADwASwBJAEQAUwA+ADwASwBJAEQAIABBAEwARwBJAEQAPQAiAEEARQBTAEMAQgBDACIAIABWAEEATABVAEUAPQAiAHAAQQA0AEYAbwBrADkAOQBWAGsARwB3AGoAZgBxAGEASgB2AHAATgBxAGcAPQA9ACIAPgA8AC8ASwBJAEQAPgA8AC8ASwBJAEQAUwA+ADwALwBQAFIATwBUAEUAQwBUAEkATgBGAE8APgA8AEwAQQBfAFUAUgBMAD4AaAB0AHQAcABzADoALwAvAHAAbABhAHkAcgBlAGEAZAB5AC4AZQB6AGQAcgBtAC4AYwBvAG0ALwBjAGUAbgBjAHkALwBwAHIAZQBhAHUAdABoAC4AYQBzAHAAeAA/AHAAWAA9AEYANgAxADQARAAxADwALwBMAEEAXwBVAFIATAA+ADwARABTAF8ASQBEAD4AVgBsAFIANwBJAGQAcwBJAEoARQB1AFIAZAAwADYATABhAHEAcwAyAGoAdwA9AD0APAAvAEQAUwBfAEkARAA+ADwALwBEAEEAVABBAD4APAAvAFcAUgBNAEgARQBBAEQARQBSAD4A";

        let pssh = PsshBox::from_base64(base64).unwrap();

        assert_eq!(
            pssh.system_id.data(),
            &[
                0x9a, 0x04, 0xf0, 0x79, 0x98, 0x40, 0x42, 0x86, //
                0xab, 0x92, 0xe6, 0x5b, 0xe0, 0x88, 0x5f, 0x95 //
            ]
        );

        assert_eq!(pssh.data_size, 670)
    }
}
