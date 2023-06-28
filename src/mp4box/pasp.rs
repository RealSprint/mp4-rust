use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use serde::Serialize;
use std::io::{Read, Seek, Write};

use crate::mp4box::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PaspBox {
    pub numerator: u32,
    pub denumerator: u32,
}

impl PaspBox {
    pub fn get_type(&self) -> BoxType {
        BoxType::PaspBox
    }

    pub fn get_size(&self) -> u64 {
        HEADER_SIZE + 8
    }
}

impl Mp4Box for PaspBox {
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
        let s = format!("aspectratio={}/{}", self.numerator, self.denumerator);
        Ok(s)
    }
}

impl<R: Read + Seek> ReadBox<&mut R> for PaspBox {
    fn read_box(reader: &mut R, size: u64) -> Result<Self> {
        let start = box_start(reader)?;

        let numerator = reader.read_u32::<BigEndian>()?;
        let denumerator = reader.read_u32::<BigEndian>()?;

        skip_bytes_to(reader, start + size)?;

        Ok(PaspBox {
            numerator,
            denumerator,
        })
    }
}

impl<W: Write> WriteBox<&mut W> for PaspBox {
    fn write_box(&self, writer: &mut W) -> Result<u64> {
        let size = self.box_size();
        BoxHeader::new(self.box_type(), size).write(writer)?;

        writer.write_u32::<BigEndian>(self.numerator)?;
        writer.write_u32::<BigEndian>(self.denumerator)?;

        Ok(size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mp4box::BoxHeader;
    use std::io::Cursor;

    #[test]
    fn test_pasp_one_one() {
        let pasp_box = PaspBox {
            numerator: 1,
            denumerator: 1,
        };
        let mut buf = Vec::new();
        pasp_box.write_box(&mut buf).unwrap();
        assert_eq!(buf.len(), pasp_box.box_size() as usize);

        let mut reader = Cursor::new(&buf);
        let header = BoxHeader::read(&mut reader).unwrap();
        assert_eq!(header.name, BoxType::PaspBox);
        assert_eq!(pasp_box.box_size(), header.size);

        let dst_box = PaspBox::read_box(&mut reader, header.size).unwrap();
        assert_eq!(pasp_box, dst_box);
    }

    #[test]
    fn test_pasp_sixteen_nine() {
        let pasp_box = PaspBox {
            numerator: 16,
            denumerator: 9,
        };
        let mut buf = Vec::new();
        pasp_box.write_box(&mut buf).unwrap();
        assert_eq!(buf.len(), pasp_box.box_size() as usize);

        let mut reader = Cursor::new(&buf);
        let header = BoxHeader::read(&mut reader).unwrap();
        assert_eq!(header.name, BoxType::PaspBox);
        assert_eq!(pasp_box.box_size(), header.size);

        let dst_box = PaspBox::read_box(&mut reader, header.size).unwrap();
        assert_eq!(pasp_box, dst_box);
    }
}
