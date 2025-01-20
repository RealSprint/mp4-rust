use std::io::{Read, Seek, Write};

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use serde::Serialize;

use super::{
    box_start, read_box_header_ext, skip_bytes_to, write_box_header_ext, BoxHeader, BoxType,
    FourCC, Mp4Box, ReadBox, Result, WriteBox, HEADER_EXT_SIZE, HEADER_SIZE,
};

const SCHM_BOX_SIZE: u64 = HEADER_SIZE + HEADER_EXT_SIZE + 4 + 4;

// ISO 14496-12:2022 - 8.12.6 Scheme Type Box
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub(crate) struct SchmBox {
    pub(crate) version: u8,
    pub(crate) flags: u32,

    pub(crate) scheme_type: FourCC,
    pub(crate) scheme_version: u32,
    pub(crate) scheme_uri: Option<String>,
}

impl SchmBox {
    pub fn get_type(&self) -> BoxType {
        BoxType::SchmBox
    }

    fn scheme_uri_size(&self) -> u64 {
        if let Some(ref scheme_uri) = self.scheme_uri {
            scheme_uri.len() as u64 + 1
        } else {
            0
        }
    }

    pub fn get_size(&self) -> u64 {
        SCHM_BOX_SIZE + self.scheme_uri_size()
    }
}

impl Mp4Box for SchmBox {
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
            "scheme_type={:?}, scheme_version={}, scheme_uri={:?}",
            self.scheme_type, self.scheme_version, self.scheme_uri
        );
        Ok(s)
    }
}

impl<R: Read + Seek> ReadBox<&mut R> for SchmBox {
    fn read_box(reader: &mut R, size: u64) -> Result<Self> {
        let start = box_start(reader)?;

        let (version, flags) = read_box_header_ext(reader)?;
        let scheme_type: FourCC = reader.read_u32::<BigEndian>()?.into();
        let scheme_version = reader.read_u32::<BigEndian>()?;

        let scheme_uri = if flags & 1 == 1 {
            let scheme_uri_size = (size - SCHM_BOX_SIZE - 1) as usize;
            let mut scheme_uri = String::with_capacity(scheme_uri_size);

            loop {
                let c = reader.read_u8()?;

                if c == 0 {
                    break;
                }
                scheme_uri.push(c.into());
            }

            Some(scheme_uri)
        } else {
            None
        };

        skip_bytes_to(reader, start + size)?;

        Ok(SchmBox {
            version,
            flags,
            scheme_type,
            scheme_version,
            scheme_uri,
        })
    }
}

impl<W: Write> WriteBox<&mut W> for SchmBox {
    fn write_box(&self, writer: &mut W) -> Result<u64> {
        let size = self.box_size();

        BoxHeader::new(self.box_type(), size).write(writer)?;
        write_box_header_ext(writer, self.version, self.flags)?;

        writer.write_u32::<BigEndian>(self.scheme_type.into())?;
        writer.write_u32::<BigEndian>(self.scheme_version)?;

        if let Some(ref scheme_uri) = self.scheme_uri {
            for c in scheme_uri.chars() {
                writer.write_u8(c as u8)?;
            }
            writer.write_u8(0)?;
        }

        Ok(size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mp4box::BoxHeader;
    use std::io::Cursor;

    #[test]
    fn test_schm_box() {
        let src_box = SchmBox {
            version: 0,
            flags: 0,
            scheme_type: FourCC { value: *b"cenc" },
            scheme_uri: None,
            scheme_version: 0x10000,
        };
        let mut buf = Vec::new();
        src_box.write_box(&mut buf).unwrap();
        assert_eq!(buf.len(), src_box.box_size() as usize);

        let expected: Vec<u8> = vec![
            0x00, 0x00, 0x00, 0x14, b's', b'c', b'h', b'm', //
            0x00, 0x00, 0x00, 0x00, b'c', b'e', b'n', b'c', //
            0x00, 0x01, 0x00, 0x00, //
        ];

        assert_eq!(buf, expected);

        let mut reader = Cursor::new(&buf);
        let header = BoxHeader::read(&mut reader).unwrap();
        assert_eq!(header.name, BoxType::SchmBox);
        assert_eq!(src_box.box_size(), header.size);

        let dst_box = SchmBox::read_box(&mut reader, header.size).unwrap();
        assert_eq!(src_box, dst_box);
    }

    #[test]
    fn test_schm_box_with_uri() {
        let src_box = SchmBox {
            version: 0,
            flags: 1,
            scheme_type: FourCC { value: *b"cenc" },
            scheme_uri: Some("https://example.com".to_string()),
            scheme_version: 0x10000,
        };
        let mut buf = Vec::new();
        src_box.write_box(&mut buf).unwrap();
        assert_eq!(buf.len(), src_box.box_size() as usize);

        let expected: Vec<u8> = vec![
            0x00, 0x00, 0x00, 0x28, b's', b'c', b'h', b'm', //
            0x00, 0x00, 0x00, 0x01, b'c', b'e', b'n', b'c', //
            0x00, 0x01, 0x00, 0x00, b'h', b't', b't', b'p', //
            b's', b':', b'/', b'/', b'e', b'x', b'a', b'm', //
            b'p', b'l', b'e', b'.', b'c', b'o', b'm', 0x00, //
        ];

        assert_eq!(buf, expected);

        let mut reader = Cursor::new(&buf);
        let header = BoxHeader::read(&mut reader).unwrap();
        assert_eq!(header.name, BoxType::SchmBox);
        assert_eq!(src_box.box_size(), header.size);

        let dst_box = SchmBox::read_box(&mut reader, header.size).unwrap();
        assert_eq!(src_box, dst_box);
    }
}
