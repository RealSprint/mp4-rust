use std::io::{Read, Seek, Write};

use serde::Serialize;

use super::{
    box_start, skip_bytes_to, tenc::TencBox, BoxHeader, BoxType, Error, Mp4Box, ReadBox, Result,
    WriteBox,
};

// ISO 14496-12:2022 - 8.12.7 Scheme Informatio nBox
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct SchiBox {
    pub(crate) tenc: TencBox,
}

impl SchiBox {
    pub fn get_type(&self) -> BoxType {
        BoxType::SchiBox
    }

    pub fn get_size(&self) -> u64 {
        8 + self.tenc.box_size()
    }
}

impl Mp4Box for SchiBox {
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

impl<R: Read + Seek> ReadBox<&mut R> for SchiBox {
    fn read_box(reader: &mut R, size: u64) -> Result<Self> {
        let start = box_start(reader)?;

        let header = BoxHeader::read(reader)?;
        let BoxHeader { name, size: s } = header;
        if s > size {
            return Err(Error::InvalidData(
                "stsd box contains a box with a larger size than it",
            ));
        }

        match name {
            BoxType::TencBox => {
                let tenc = TencBox::read_box(reader, s)?;

                skip_bytes_to(reader, start + size)?;

                Ok(SchiBox { tenc })
            }
            _ => Err(Error::InvalidData("Invalid box type in SchiBox")),
        }
    }
}

impl<W: Write> WriteBox<&mut W> for SchiBox {
    fn write_box(&self, writer: &mut W) -> Result<u64> {
        let size = self.box_size();
        BoxHeader::new(self.box_type(), size).write(writer)?;

        self.tenc.write_box(writer)?;

        Ok(size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mp4box::BoxHeader;
    use std::io::Cursor;

    #[test]
    fn test_schi() {
        let src_box = SchiBox {
            tenc: TencBox::new_unprotected(),
        };

        let mut buf = Vec::new();
        src_box.write_box(&mut buf).unwrap();
        assert_eq!(buf.len(), src_box.box_size() as usize);

        let expected: Vec<u8> = vec![
            0x00, 0x00, 0x00, 0x28, b's', b'c', b'h', b'i', //
            0x00, 0x00, 0x00, 0x20, b't', b'e', b'n', b'c', //
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
        ];
        assert_eq!(buf, expected);

        let mut reader = Cursor::new(&buf);
        let header = BoxHeader::read(&mut reader).unwrap();
        assert_eq!(header.name, BoxType::SchiBox);
        assert_eq!(src_box.box_size(), header.size);

        let dst_box = SchiBox::read_box(&mut reader, header.size).unwrap();
        assert_eq!(src_box, dst_box);
    }
}
