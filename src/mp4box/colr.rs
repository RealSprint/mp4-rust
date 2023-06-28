use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use bytes::BytesMut;
use serde::Serialize;
use std::io::{Read, Seek, Write};
use std::str::from_utf8;

use crate::mp4box::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum Color {
    Nclx(ColorConfig),
    #[serde(skip_serializing)]
    Prof(Bytes),
}

impl Color {
    pub fn get_size(&self) -> u64 {
        match self {
            Color::Nclx(_) => 3 * 2 + 1,
            Color::Prof(nclx) => nclx.len() as u64,
        }
    }

    pub fn tag(&self) -> &str {
        match self {
            Color::Nclx(_) => "nclx",
            Color::Prof(_) => "prof",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ColrBox {
    pub color_config: Color,
}

impl ColrBox {
    pub fn get_type(&self) -> BoxType {
        BoxType::ColrBox
    }

    pub fn get_size(&self) -> u64 {
        HEADER_SIZE + 4 + self.color_config.get_size()
    }
}

impl Mp4Box for ColrBox {
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
        let s = format!("color={:?}", self.color_config);
        Ok(s)
    }
}

impl<R: Read + Seek> ReadBox<&mut R> for ColrBox {
    fn read_box(reader: &mut R, size: u64) -> Result<Self> {
        let start = box_start(reader)?;

        let mut tag = [0u8; 4];
        reader.read_exact(&mut tag)?;

        let color_config = match from_utf8(&tag).map_err(|_| Error::InvalidData("invalid colr tag"))
        {
            Ok("prof") => {
                let position = reader.stream_position()?;
                let mut icc = BytesMut::new();
                icc.resize((size - position) as usize, 0);
                reader.read_exact(icc.as_mut())?;
                Color::Prof(icc.into())
            }
            Ok("nclx") => Color::Nclx(ColorConfig {
                color_primaries: reader.read_u16::<BigEndian>()?,
                transfer_characteristics: reader.read_u16::<BigEndian>()?,
                matrix_coefficients: reader.read_u16::<BigEndian>()?,
                full_range: reader.read_u8()? >> 7 > 0,
            }),
            _ => {
                return Err(Error::InvalidData("invalid colr tag"));
            }
        };

        skip_bytes_to(reader, start + size)?;

        Ok(ColrBox { color_config })
    }
}

impl<W: Write> WriteBox<&mut W> for ColrBox {
    fn write_box(&self, writer: &mut W) -> Result<u64> {
        let size = self.box_size();
        BoxHeader::new(self.box_type(), size).write(writer)?;

        writer.write_all(self.color_config.tag().as_bytes())?;

        match &self.color_config {
            Color::Nclx(colr) => {
                writer.write_u16::<BigEndian>(colr.color_primaries)?;
                writer.write_u16::<BigEndian>(colr.transfer_characteristics)?;
                writer.write_u16::<BigEndian>(colr.matrix_coefficients)?;
                writer.write_u8(if colr.full_range { 1 << 7 } else { 0 << 7 })?;
            }
            Color::Prof(icc) => {
                writer.write_all(icc)?;
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
    fn test_colr_nclx() {
        let colr_box = ColrBox {
            color_config: Color::Nclx(ColorConfig {
                color_primaries: 1,
                transfer_characteristics: 1,
                matrix_coefficients: 1,
                full_range: false,
            }),
        };
        let mut buf = Vec::new();
        colr_box.write_box(&mut buf).unwrap();
        assert_eq!(buf.len(), colr_box.box_size() as usize);

        let mut reader = Cursor::new(&buf);
        let header = BoxHeader::read(&mut reader).unwrap();
        assert_eq!(header.name, BoxType::ColrBox);
        assert_eq!(colr_box.box_size(), header.size);

        let dst_box = ColrBox::read_box(&mut reader, header.size).unwrap();
        assert_eq!(colr_box, dst_box);
    }

    #[test]
    fn test_colr_prof() {
        let colr_box = ColrBox {
            color_config: Color::Prof(vec![0u8; 10].into()),
        };
        let mut buf = Vec::new();
        colr_box.write_box(&mut buf).unwrap();
        assert_eq!(buf.len(), colr_box.box_size() as usize);

        let mut reader = Cursor::new(&buf);
        let header = BoxHeader::read(&mut reader).unwrap();
        assert_eq!(header.name, BoxType::ColrBox);
        assert_eq!(colr_box.box_size(), header.size);

        let dst_box = ColrBox::read_box(&mut reader, header.size).unwrap();
        assert_eq!(colr_box, dst_box);
    }
}
