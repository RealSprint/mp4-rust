use std::io::{Read, Seek, Write};

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use serde::Serialize;

use super::{
    box_start, skip_bytes_to, BoxHeader, BoxType, FourCC, Mp4Box, ReadBox, Result, WriteBox,
    HEADER_SIZE,
};

// ISO 14496-12:2022 - 8.12.3 Original Format Box
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub(crate) struct FrmaBox {
    pub(crate) data_format: FourCC,
}

impl FrmaBox {
    pub fn get_type(&self) -> BoxType {
        BoxType::FrmaBox
    }

    pub fn get_size(&self) -> u64 {
        HEADER_SIZE + 4
    }
}

impl Mp4Box for FrmaBox {
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
        Ok(format!("format={}", self.data_format))
    }
}

impl<R: Read + Seek> ReadBox<&mut R> for FrmaBox {
    fn read_box(reader: &mut R, size: u64) -> Result<Self> {
        let start = box_start(reader)?;

        let data_format = reader.read_u32::<BigEndian>()?.into();

        skip_bytes_to(reader, start + size)?;
        Ok(FrmaBox { data_format })
    }
}

impl<W: Write> WriteBox<&mut W> for FrmaBox {
    fn write_box(&self, writer: &mut W) -> Result<u64> {
        let size = self.box_size();

        BoxHeader::new(self.box_type(), size).write(writer)?;

        writer.write_u32::<BigEndian>(self.data_format.into())?;

        Ok(size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mp4box::BoxHeader;
    use std::io::Cursor;

    #[test]
    fn test_frma() {
        let src_box = FrmaBox {
            data_format: FourCC { value: *b"avc1" },
        };

        let mut buf = Vec::new();
        src_box.write_box(&mut buf).unwrap();
        assert_eq!(buf.len(), src_box.box_size() as usize);

        let expected: Vec<u8> = vec![
            0x00, 0x00, 0x00, 0x0c, b'f', b'r', b'm', b'a', b'a', b'v', b'c', b'1',
        ];
        assert_eq!(buf, expected);

        let mut reader = Cursor::new(&buf);
        let header = BoxHeader::read(&mut reader).unwrap();
        assert_eq!(header.name, BoxType::FrmaBox);
        assert_eq!(src_box.box_size(), header.size);

        let dst_box = FrmaBox::read_box(&mut reader, header.size).unwrap();
        assert_eq!(src_box, dst_box);
    }
}
