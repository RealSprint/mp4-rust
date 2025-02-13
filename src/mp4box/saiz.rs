use std::io::{Read, Seek, Write};

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use serde::Serialize;

use super::{
    box_start, read_box_header_ext, skip_bytes_to, write_box_header_ext, BoxHeader, BoxType,
    Mp4Box, Mp4Context, ReadBox, Result, WriteBox, HEADER_EXT_SIZE, HEADER_SIZE,
};

// ISO 23001-7:2023 - 8.2 Track Encryption Box
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct SaizBox {
    aux_info_type: Option<u32>,
    aux_info_type_parameter: Option<u32>,
    default_sample_info_size: u8,
    sample_count: u32,
    sample_info_sizes: Vec<u8>,
}

impl SaizBox {
    pub fn new(default_sample_info_size: u8) -> Self {
        SaizBox {
            aux_info_type: None,
            aux_info_type_parameter: None,
            default_sample_info_size,
            sample_count: 0,
            sample_info_sizes: vec![],
        }
    }

    pub fn increment_sample_count(&mut self) {
        self.sample_count += 1;
    }

    pub fn get_type(&self) -> BoxType {
        BoxType::SaizBox
    }

    pub fn get_size(&self) -> u64 {
        let mut size = HEADER_SIZE + HEADER_EXT_SIZE + 1 + 4;
        if let (Some(_), Some(_)) = (self.aux_info_type, self.aux_info_type_parameter) {
            size += 8
        }

        if self.default_sample_info_size == 0 {
            size += self.sample_count as u64;
        }

        size
    }
}

impl Mp4Box for SaizBox {
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
        Ok(format!("aux_info_type: {:?}, ", self.aux_info_type))
    }
}

impl<R: Read + Seek> ReadBox<&mut R> for SaizBox {
    fn read_box(reader: &mut R, size: u64, _context: &mut Mp4Context) -> Result<Self> {
        let start = box_start(reader)?;

        let (_version, flags) = read_box_header_ext(reader)?;

        let (aux_info_type, aux_info_type_parameter) = if flags & 1 == 1 {
            let aux_info_type = reader.read_u32::<BigEndian>()?;
            let aux_info_type_parameter = reader.read_u32::<BigEndian>()?;

            (Some(aux_info_type), Some(aux_info_type_parameter))
        } else {
            (None, None)
        };

        let default_sample_info_size = reader.read_u8()?;
        let sample_count = reader.read_u32::<BigEndian>()?;
        let mut sample_info_sizes = Vec::new();
        if default_sample_info_size == 0 {
            for _ in 0..sample_count {
                sample_info_sizes.push(reader.read_u8()?);
            }
        }

        skip_bytes_to(reader, start + size)?;

        Ok(SaizBox {
            aux_info_type,
            aux_info_type_parameter,
            default_sample_info_size,
            sample_count,
            sample_info_sizes,
        })
    }
}

impl<W: Write> WriteBox<&mut W> for SaizBox {
    fn write_box(&self, writer: &mut W) -> Result<u64> {
        let size = self.box_size();

        BoxHeader::new(self.box_type(), size).write(writer)?;

        let flags = if self.aux_info_type.is_some() && self.aux_info_type_parameter.is_some() {
            1
        } else {
            0
        };

        write_box_header_ext(writer, 0, flags)?;

        if let (Some(aux_info_type), Some(aux_info_type_parameter)) =
            (self.aux_info_type, self.aux_info_type_parameter)
        {
            writer.write_u32::<BigEndian>(aux_info_type)?;
            writer.write_u32::<BigEndian>(aux_info_type_parameter)?;
        }

        writer.write_u8(self.default_sample_info_size)?;
        writer.write_u32::<BigEndian>(self.sample_count)?;

        if self.default_sample_info_size == 0 {
            for sample_info_size in &self.sample_info_sizes {
                writer.write_u8(*sample_info_size)?;
            }
        }

        Ok(size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mp4box::BoxHeader;
    use std::{io::Cursor, vec};

    #[test]
    fn test_saiz_plain() {
        let src_box = SaizBox {
            aux_info_type: None,
            aux_info_type_parameter: None,
            default_sample_info_size: 8,
            sample_count: 0,
            sample_info_sizes: vec![],
        };

        let mut buf = Vec::new();
        src_box.write_box(&mut buf).unwrap();
        assert_eq!(buf.len(), src_box.box_size() as usize);

        let expected = vec![
            0x00, 0x00, 0x00, 0x11, b's', b'a', b'i', b'z', // header
            0x00, 0x00, 0x00, 0x00, // ext header
            0x08, 0x00, 0x00, 0x00, 0x00,
        ];

        assert_eq!(buf, expected);

        let mut reader = Cursor::new(&buf);
        let header = BoxHeader::read(&mut reader).unwrap();
        assert_eq!(header.name, BoxType::SaizBox);
        assert_eq!(src_box.box_size(), header.size);

        let dst_box =
            SaizBox::read_box(&mut reader, header.size, &mut Mp4Context::default()).unwrap();
        assert_eq!(src_box, dst_box);
    }

    #[test]
    fn test_saiz_aux_info() {
        let src_box = SaizBox {
            aux_info_type: Some(1),
            aux_info_type_parameter: Some(1),
            default_sample_info_size: 8,
            sample_count: 0,
            sample_info_sizes: vec![],
        };

        let mut buf = Vec::new();
        src_box.write_box(&mut buf).unwrap();
        assert_eq!(buf.len(), src_box.box_size() as usize);

        let expected = vec![
            0x00, 0x00, 0x00, 0x19, b's', b'a', b'i', b'z', // header
            0x00, 0x00, 0x00, 0x01, // ext header
            0x00, 0x00, 0x00, 0x01, // aux_info_type
            0x00, 0x00, 0x00, 0x01, // aux_info_type_parameter
            0x08, 0x00, 0x00, 0x00, 0x00,
        ];

        assert_eq!(buf, expected);

        let mut reader = Cursor::new(&buf);
        let header = BoxHeader::read(&mut reader).unwrap();
        assert_eq!(header.name, BoxType::SaizBox);
        assert_eq!(src_box.box_size(), header.size);

        let dst_box =
            SaizBox::read_box(&mut reader, header.size, &mut Mp4Context::default()).unwrap();
        assert_eq!(src_box, dst_box);
    }

    #[test]
    fn test_saiz_samples_sizes() {
        let src_box = SaizBox {
            aux_info_type: None,
            aux_info_type_parameter: None,
            default_sample_info_size: 0,
            sample_count: 2,
            sample_info_sizes: vec![1, 2],
        };

        let mut buf = Vec::new();
        src_box.write_box(&mut buf).unwrap();
        assert_eq!(buf.len(), src_box.box_size() as usize);

        let expected = vec![
            0x00, 0x00, 0x00, 0x13, b's', b'a', b'i', b'z', // header
            0x00, 0x00, 0x00, 0x00, // ext header
            0x00, 0x00, 0x00, 0x00, 0x02, //
            0x01, 0x02,
        ];

        assert_eq!(buf, expected);

        let mut reader = Cursor::new(&buf);
        let header = BoxHeader::read(&mut reader).unwrap();
        assert_eq!(header.name, BoxType::SaizBox);
        assert_eq!(src_box.box_size(), header.size);

        let dst_box =
            SaizBox::read_box(&mut reader, header.size, &mut Mp4Context::default()).unwrap();
        assert_eq!(src_box, dst_box);
    }
}
