use std::io::{Read, Seek, Write};

use byteorder::{ReadBytesExt, WriteBytesExt};
use serde::Serialize;

use super::{
    box_start, read_box_header_ext, skip_bytes_to, write_box_header_ext, BoxHeader, BoxType, Error,
    Mp4Box, ReadBox, Result, WriteBox, HEADER_EXT_SIZE, HEADER_SIZE,
};

// ISO 23001-7:2023 - 8.2 Track Encryption Box
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct TencBox {
    default_crypt_byte_block: Option<u8>,
    default_skip_byte_block: Option<u8>,

    default_is_protected: bool,
    // 0: no encryption or constant IVs
    // 8: 64-bit IVs
    // 16: 128-bit IVs
    default_per_sample_iv_size: u8,
    default_kid: [u8; 16],

    // 8: 64-bit IVs
    // 16: 128-bit IVs
    default_constant_iv_size: Option<u8>,
    default_constant_iv: Option<[u8; 16]>,
}

impl TencBox {
    pub fn new_unprotected() -> Self {
        TencBox {
            default_crypt_byte_block: None,
            default_skip_byte_block: None,

            default_is_protected: false,
            default_per_sample_iv_size: 0,
            default_kid: [0; 16],

            default_constant_iv_size: None,
            default_constant_iv: None,
        }
    }

    pub fn new_kid_protected(iv: InitializationVector) -> Self {
        TencBox {
            default_crypt_byte_block: None,
            default_skip_byte_block: None,

            default_is_protected: true,
            default_per_sample_iv_size: iv.size,
            default_kid: iv.data,

            default_constant_iv_size: None,
            default_constant_iv: None,
        }
    }

    pub fn new_constant_iv_protected(iv: InitializationVector) -> Self {
        TencBox {
            default_crypt_byte_block: None,
            default_skip_byte_block: None,

            default_is_protected: true,
            default_per_sample_iv_size: 0,
            default_kid: [0; 16],

            default_constant_iv_size: Some(iv.size),
            default_constant_iv: Some(iv.data),
        }
    }

    pub fn get_type(&self) -> BoxType {
        BoxType::TencBox
    }

    pub fn get_size(&self) -> u64 {
        let base_size = HEADER_SIZE + HEADER_EXT_SIZE + 1 + 1 + 1 + 1 + 16;

        let dynamic_size = self
            .default_constant_iv_size
            .as_ref()
            .map(|s| s + 1)
            .unwrap_or(0) as u64;

        base_size + dynamic_size
    }
}

impl Mp4Box for TencBox {
    fn box_type(&self) -> BoxType {
        self.get_type()
    }

    fn box_size(&self) -> u64 {
        self.get_size()
    }

    fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string(&self).unwrap())
    }

    fn summary(&self) -> Result<String> {
        Ok(format!(
            "default_crypt_byte_block={:?}, default_skip_byte_block={:?}, default_is_protected={}, default_per_sample_iv_size={}, default_kid={:?}, default_constant_iv_size={:?}, default_constant_iv={:?}",
            self.default_crypt_byte_block, self.default_skip_byte_block, self.default_is_protected, self.default_per_sample_iv_size, self.default_kid, self.default_constant_iv_size, self.default_constant_iv
        ))
    }
}

impl<R: Read + Seek> ReadBox<&mut R> for TencBox {
    fn read_box(reader: &mut R, size: u64) -> Result<Self> {
        let start = box_start(reader)?;

        let (version, _flags) = read_box_header_ext(reader)?;

        // reserved
        reader.read_u8()?;

        let temp = reader.read_u8()?;
        let (default_crypt_byte_block, default_skip_byte_block) = if version != 0 {
            let default_crypt_byte_block = temp & 0x0F;
            let default_skip_byte_block = (temp & 0xF0) >> 4;

            (
                Some(default_crypt_byte_block),
                Some(default_skip_byte_block),
            )
        } else {
            (None, None)
        };

        // 0x00: not protected
        // 0x01: protected
        // 0x02 – 0xFF: reserved
        let default_is_protected = reader.read_u8()? == 1;

        let default_per_sample_iv_size = reader.read_u8()?;

        let mut default_kid = [0; 16];
        reader.read_exact(&mut default_kid)?;

        let (default_constant_iv_size, default_constant_iv) =
            if default_is_protected && default_per_sample_iv_size == 0 {
                let default_constant_iv_size = reader.read_u8()?;
                let mut default_constant_iv = [0; 16];

                #[allow(clippy::needless_range_loop)]
                for i in 0..default_constant_iv_size as usize {
                    default_constant_iv[i] = reader.read_u8()?;
                }

                (Some(default_constant_iv_size), Some(default_constant_iv))
            } else {
                (None, None)
            };

        skip_bytes_to(reader, start + size)?;

        Ok(TencBox {
            default_crypt_byte_block,
            default_skip_byte_block,
            default_is_protected,
            default_per_sample_iv_size,
            default_kid,
            default_constant_iv_size,
            default_constant_iv,
        })
    }
}

impl<W: Write> WriteBox<&mut W> for TencBox {
    fn write_box(&self, writer: &mut W) -> Result<u64> {
        let size = self.box_size();

        BoxHeader::new(self.box_type(), size).write(writer)?;

        let version =
            if self.default_skip_byte_block.is_some() && self.default_crypt_byte_block.is_some() {
                1
            } else {
                0
            };

        write_box_header_ext(writer, version, 0)?;

        // reserved
        writer.write_u8(0)?;

        let temp = match (self.default_skip_byte_block, self.default_crypt_byte_block) {
            (Some(skip), Some(crypt)) => (skip << 4) | (crypt),
            _ => 0,
        };

        writer.write_u8(temp)?;

        writer.write_u8(if self.default_is_protected { 1 } else { 0 })?;

        writer.write_u8(self.default_per_sample_iv_size)?;

        writer.write_all(&self.default_kid)?;

        if self.default_is_protected && self.default_per_sample_iv_size == 0 {
            match (&self.default_constant_iv_size, &self.default_constant_iv) {
                (Some(size), Some(iv)) => {
                    writer.write_u8(*size)?;
                    for i in 0..*size {
                        writer.write_u8(iv[i as usize])?;
                    }
                }
                _ => {
                    return Err(Error::InvalidData(
                        "default_constant_iv_size and default_constant_iv must be set when default_is_protected is true and default_per_sample_iv_size is 0",
                    ));
                }
            }
        }

        Ok(size)
    }
}

pub struct InitializationVector {
    size: u8,
    data: [u8; 16],
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mp4box::BoxHeader;
    use std::io::Cursor;

    #[test]
    fn test_tenc_unprotected() {
        let src_box = TencBox::new_unprotected();

        let mut buf = Vec::new();
        src_box.write_box(&mut buf).unwrap();
        assert_eq!(buf.len(), src_box.box_size() as usize);

        let expected = vec![
            0x00, 0x00, 0x00, 0x20, b't', b'e', b'n', b'c', //
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
        ];

        assert_eq!(buf, expected);

        let mut reader = Cursor::new(&buf);
        let header = BoxHeader::read(&mut reader).unwrap();
        assert_eq!(header.name, BoxType::TencBox);
        assert_eq!(src_box.box_size(), header.size);

        let dst_box = TencBox::read_box(&mut reader, header.size).unwrap();
        assert_eq!(src_box, dst_box);
    }

    #[test]
    fn test_tenc_kid_64() {
        let data = [
            0x6d, 0x76, 0xf2, 0x5c, 0xb1, 0x7f, 0x5e, 0x16, //
        ];
        let src_box = TencBox::new_kid_protected(InitializationVector::new_64_bit(data));

        let mut buf = Vec::new();
        src_box.write_box(&mut buf).unwrap();
        assert_eq!(buf.len(), src_box.box_size() as usize);

        let expected = vec![
            0x00, 0x00, 0x00, 0x20, b't', b'e', b'n', b'c', //
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x08, //
            0x6d, 0x76, 0xf2, 0x5c, 0xb1, 0x7f, 0x5e, 0x16, //
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
        ];

        assert_eq!(buf, expected);

        let mut reader = Cursor::new(&buf);
        let header = BoxHeader::read(&mut reader).unwrap();
        assert_eq!(header.name, BoxType::TencBox);
        assert_eq!(src_box.box_size(), header.size);

        let dst_box = TencBox::read_box(&mut reader, header.size).unwrap();
        assert_eq!(src_box, dst_box);
    }

    #[test]
    fn test_tenc_kid_128() {
        let data = [
            0x6d, 0x76, 0xf2, 0x5c, 0xb1, 0x7f, 0x5e, 0x16, //
            0xb8, 0xea, 0xef, 0x6b, 0xbf, 0x58, 0x2d, 0x8e, //
        ];
        let src_box = TencBox::new_kid_protected(InitializationVector::new_128_bit(data));

        let mut buf = Vec::new();
        src_box.write_box(&mut buf).unwrap();
        assert_eq!(buf.len(), src_box.box_size() as usize);

        let expected = vec![
            0x00, 0x00, 0x00, 0x20, b't', b'e', b'n', b'c', //
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x10, //
            0x6d, 0x76, 0xf2, 0x5c, 0xb1, 0x7f, 0x5e, 0x16, //
            0xb8, 0xea, 0xef, 0x6b, 0xbf, 0x58, 0x2d, 0x8e, //
        ];

        assert_eq!(buf, expected);

        let mut reader = Cursor::new(&buf);
        let header = BoxHeader::read(&mut reader).unwrap();
        assert_eq!(header.name, BoxType::TencBox);
        assert_eq!(src_box.box_size(), header.size);

        let dst_box = TencBox::read_box(&mut reader, header.size).unwrap();
        assert_eq!(src_box, dst_box);
    }

    #[test]
    fn test_tenc_constant_iv_64() {
        let data = [
            0x6d, 0x76, 0xf2, 0x5c, 0xb1, 0x7f, 0x5e, 0x16, //
        ];
        let src_box = TencBox::new_constant_iv_protected(InitializationVector::new_64_bit(data));

        let mut buf = Vec::new();
        src_box.write_box(&mut buf).unwrap();
        assert_eq!(buf.len(), src_box.box_size() as usize);

        let expected = vec![
            0x00, 0x00, 0x00, 0x29, b't', b'e', b'n', b'c', //
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, //
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
            0x08, 0x6d, 0x76, 0xf2, 0x5c, 0xb1, 0x7f, 0x5e, //
            0x16,
        ];

        assert_eq!(buf, expected);

        let mut reader = Cursor::new(&buf);
        let header = BoxHeader::read(&mut reader).unwrap();
        assert_eq!(header.name, BoxType::TencBox);
        assert_eq!(src_box.box_size(), header.size);

        let dst_box = TencBox::read_box(&mut reader, header.size).unwrap();
        assert_eq!(src_box, dst_box);
    }

    #[test]
    fn test_tenc_constant_iv_128() {
        let data = [
            0x6d, 0x76, 0xf2, 0x5c, 0xb1, 0x7f, 0x5e, 0x16, //
            0xb8, 0xea, 0xef, 0x6b, 0xbf, 0x58, 0x2d, 0x8e, //
        ];
        let src_box = TencBox::new_constant_iv_protected(InitializationVector::new_128_bit(data));

        let mut buf = Vec::new();
        src_box.write_box(&mut buf).unwrap();
        assert_eq!(buf.len(), src_box.box_size() as usize);

        let expected = vec![
            0x00, 0x00, 0x00, 0x31, b't', b'e', b'n', b'c', //
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, //
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
            0x10, 0x6d, 0x76, 0xf2, 0x5c, 0xb1, 0x7f, 0x5e, //
            0x16, 0xb8, 0xea, 0xef, 0x6b, 0xbf, 0x58, 0x2d, //
            0x8e,
        ];

        assert_eq!(buf, expected);

        let mut reader = Cursor::new(&buf);
        let header = BoxHeader::read(&mut reader).unwrap();
        assert_eq!(header.name, BoxType::TencBox);
        assert_eq!(src_box.box_size(), header.size);

        let dst_box = TencBox::read_box(&mut reader, header.size).unwrap();
        assert_eq!(src_box, dst_box);
    }
}
