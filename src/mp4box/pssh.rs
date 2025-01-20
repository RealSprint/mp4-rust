use std::io::{Read, Seek, Write};

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use serde::Serialize;

use super::{
    box_start, skip_bytes_to, write_box_header_ext, BoxHeader, BoxType, Error, Mp4Box, ReadBox,
    Result, WriteBox, HEADER_EXT_SIZE, HEADER_SIZE,
};

// ISO 23001-7:2023 - 8.1 Protection System Specific Header Box
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct PsshBox {
    version: u8,
    flags: u32,

    system_id: [u8; 16],

    kid_count: Option<u32>,
    kid: Vec<[u8; 16]>,

    data_size: u32,
    data: Vec<u8>,
}

impl PsshBox {
    pub fn new(system_id: [u8; 16], data: Vec<u8>) -> Self {
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

    pub fn with_kid(system_id: [u8; 16], kid: Vec<[u8; 16]>, data: Vec<u8>) -> Self {
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
            4 + 16 * self.kid_count.unwrap() as u64
        } else {
            0
        };

        let data_size = 4 + self.data_size as u64;

        HEADER_SIZE + HEADER_EXT_SIZE + 16 + kid_size + data_size
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
        Ok(serde_json::to_string(&self).unwrap())
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
    fn read_box(reader: &mut R, size: u64) -> Result<Self> {
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
            (Some(kid_count), kid)
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
            system_id,
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

        writer.write_all(&self.system_id)?;

        if self.version > 0 {
            let kid_count = match self.kid_count {
                Some(kid_count) => kid_count,
                None => return Err(Error::InvalidData("kid_count is required for version > 0")),
            };

            writer.write_u32::<BigEndian>(kid_count)?;

            for i in 0..kid_count {
                writer.write_all(&self.kid[i as usize])?;
            }
        }

        writer.write_u32::<BigEndian>(self.data_size)?;
        writer.write_all(&self.data)?;

        Ok(size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mp4box::BoxHeader;
    use std::io::Cursor;

    #[test]
    fn test_pssh() {
        let system_id = [
            0x10, 0x77, 0xef, 0xec, 0xc0, 0xb2, 0x4d, 0x02, //
            0xac, 0xe3, 0x3c, 0x1e, 0x52, 0xe2, 0xfb, 0x4b,
        ];

        let data = vec![
            0x6d, 0x76, 0xf2, 0x5c, 0xb1, 0x7f, 0x5e, 0x16, //
        ];

        let src_box = PsshBox::new(system_id, data);

        let mut buf = Vec::new();
        src_box.write_box(&mut buf).unwrap();
        assert_eq!(buf.len(), src_box.box_size() as usize);

        let expected: Vec<u8> = vec![
            0x00, 0x00, 0x00, 0x34, b'p', b's', b's', b'h', //
            0x01, 0x00, 0x00, 0x00, 0x10, 0x77, 0xef, 0xec, //
            0xc0, 0xb2, 0x4d, 0x02, 0xac, 0xe3, 0x3c, 0x1e, //
            0x52, 0xe2, 0xfb, 0x4b, 0x00, 0x00, 0x00, 0x01, //
            0x00, 0x00, 0x00, 0x00,
        ];
        assert_eq!(buf, expected);

        let mut reader = Cursor::new(&buf);
        let header = BoxHeader::read(&mut reader).unwrap();
        assert_eq!(header.name, BoxType::PsshBox);
        assert_eq!(src_box.box_size(), header.size);

        let dst_box = PsshBox::read_box(&mut reader, header.size).unwrap();
        assert_eq!(src_box, dst_box);
    }

    #[test]
    fn test_pssh_with_kid() {
        let kid = vec![[
            0x6d, 0x76, 0xf2, 0x5c, 0xb1, 0x7f, 0x5e, 0x16, //
            0xb8, 0xea, 0xef, 0x6b, 0xbf, 0x58, 0x2d, 0x8e,
        ]];

        let system_id = [
            0x10, 0x77, 0xef, 0xec, 0xc0, 0xb2, 0x4d, 0x02, //
            0xac, 0xe3, 0x3c, 0x1e, 0x52, 0xe2, 0xfb, 0x4b,
        ];

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

        let dst_box = PsshBox::read_box(&mut reader, header.size).unwrap();
        assert_eq!(src_box, dst_box);
    }
}
