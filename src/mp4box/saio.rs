use std::{
    io::{Read, Seek, Write},
    vec,
};

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use serde::Serialize;

use super::{
    box_start, read_box_header_ext, skip_bytes_to, write_box_header_ext, BoxHeader, BoxType,
    Mp4Box, Mp4Context, ReadBox, Result, WriteBox, HEADER_EXT_SIZE, HEADER_SIZE,
};

// ISO 23001-7:2023 - 8.2 Track Encryption Box
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct SaioBox {
    aux_info_type: Option<u32>,
    aux_info_type_parameter: Option<u32>,
    version: u8,
    offsets: Vec<u64>,
}

impl SaioBox {
    pub fn new(version: u8) -> Self {
        SaioBox {
            aux_info_type: None,
            aux_info_type_parameter: None,
            version,
            offsets: vec![],
        }
    }

    pub fn set_single_offset(&mut self, offset: u64) {
        self.offsets = vec![offset];
    }

    pub fn get_type(&self) -> BoxType {
        BoxType::SaioBox
    }

    pub fn get_size(&self) -> u64 {
        let mut size = HEADER_SIZE + HEADER_EXT_SIZE;

        if self.aux_info_type.is_some() && self.aux_info_type_parameter.is_some() {
            size += 8;
        }

        // entry_count
        size += 4;

        if self.version == 0 {
            size += self.offsets.len() as u64 * 4;
        } else {
            size += self.offsets.len() as u64 * 8;
        }

        size
    }
}

impl Mp4Box for SaioBox {
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
        Ok("".to_string())
    }
}

impl<R: Read + Seek> ReadBox<&mut R> for SaioBox {
    fn read_box(reader: &mut R, size: u64, _context: &mut Mp4Context) -> Result<Self> {
        let start = box_start(reader)?;

        let (version, flags) = read_box_header_ext(reader)?;

        let (aux_info_type, aux_info_type_parameter) = if flags & 1 == 1 {
            let aux_info_type = reader.read_u32::<BigEndian>()?;
            let aux_info_type_parameter = reader.read_u32::<BigEndian>()?;

            (Some(aux_info_type), Some(aux_info_type_parameter))
        } else {
            (None, None)
        };

        let entry_count = reader.read_u32::<BigEndian>()?;
        let mut offsets = vec![];

        for _ in 0..entry_count {
            let offset = if version == 0 {
                reader.read_u32::<BigEndian>()? as u64
            } else {
                reader.read_u64::<BigEndian>()?
            };
            offsets.push(offset);
        }

        skip_bytes_to(reader, start + size)?;

        Ok(SaioBox {
            aux_info_type,
            aux_info_type_parameter,
            offsets,
            version,
        })
    }
}

impl<W: Write> WriteBox<&mut W> for SaioBox {
    fn write_box(&self, writer: &mut W) -> Result<u64> {
        let size = self.box_size();

        BoxHeader::new(self.box_type(), size).write(writer)?;

        let flags = if self.aux_info_type.is_some() && self.aux_info_type_parameter.is_some() {
            1
        } else {
            0
        };

        write_box_header_ext(writer, self.version, flags)?;

        if let (Some(aux_info_type), Some(aux_info_type_parameter)) =
            (self.aux_info_type, self.aux_info_type_parameter)
        {
            writer.write_u32::<BigEndian>(aux_info_type)?;
            writer.write_u32::<BigEndian>(aux_info_type_parameter)?;
        }

        writer.write_u32::<BigEndian>(self.offsets.len() as u32)?;

        for offset in &self.offsets {
            if self.version == 0 {
                writer.write_u32::<BigEndian>(*offset as u32)?;
            } else {
                writer.write_u64::<BigEndian>(*offset)?;
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
    fn test_saio() {
        let src_box = SaioBox {
            aux_info_type: None,
            aux_info_type_parameter: None,
            offsets: vec![3745],
            version: 0,
        };

        let mut buf = Vec::new();
        src_box.write_box(&mut buf).unwrap();
        assert_eq!(buf.len(), src_box.box_size() as usize);

        let expected = vec![
            0x00, 0x00, 0x00, 0x14, 0x73, 0x61, 0x69, 0x6f, // header
            0x00, 0x00, 0x00, 0x00, // ext header
            0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x0e, 0xa1,
        ];

        assert_eq!(buf, expected);

        let mut reader = Cursor::new(&buf);
        let header = BoxHeader::read(&mut reader).unwrap();
        assert_eq!(header.name, BoxType::SaioBox);
        assert_eq!(src_box.box_size(), header.size);

        let dst_box =
            SaioBox::read_box(&mut reader, header.size, &mut Mp4Context::default()).unwrap();
        assert_eq!(src_box, dst_box);
    }
}
