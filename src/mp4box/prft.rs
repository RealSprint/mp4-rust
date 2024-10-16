use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use serde::Serialize;
use std::io::{Read, Seek, Write};

use crate::mp4box::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PrftBox {
    pub version: u8,
    pub flags: u32,
    pub reference_track_id: u32,
    pub ntp_timestamp: u64,
    pub media_time: u64,
}

impl Default for PrftBox {
    fn default() -> Self {
        PrftBox {
            version: 1,
            flags: 0,
            reference_track_id: 0,
            ntp_timestamp: 0,
            media_time: 0,
        }
    }
}

impl PrftBox {
    pub fn get_type(&self) -> BoxType {
        BoxType::PrftBox
    }

    pub fn get_size(&self) -> u64 {
        let mut size = HEADER_SIZE + HEADER_EXT_SIZE + 4 + 8;
        if self.version == 1 {
            size += 8;
        } else if self.version == 0 {
            size += 4;
        }
        size
    }
}

impl Mp4Box for PrftBox {
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
            "ntp_timestamp={} media_time={}",
            self.ntp_timestamp, self.media_time
        );
        Ok(s)
    }
}

impl<R: Read + Seek> ReadBox<&mut R> for PrftBox {
    fn read_box(reader: &mut R, size: u64) -> Result<Self> {
        let start = box_start(reader)?;

        let (version, flags) = read_box_header_ext(reader)?;

        let reference_track_id = reader.read_u32::<BigEndian>()?;
        let ntp_timestamp = reader.read_u64::<BigEndian>()?;

        let media_time = if version == 0 {
            reader.read_u32::<BigEndian>()? as u64
        } else {
            reader.read_u64::<BigEndian>()?
        };

        skip_bytes_to(reader, start + size)?;

        Ok(PrftBox {
            version,
            flags,
            ntp_timestamp,
            reference_track_id,
            media_time,
        })
    }
}

impl<W: Write> WriteBox<&mut W> for PrftBox {
    fn write_box(&self, writer: &mut W) -> Result<u64> {
        let size = self.box_size();
        BoxHeader::new(self.box_type(), size).write(writer)?;

        write_box_header_ext(writer, self.version, self.flags)?;

        writer.write_u32::<BigEndian>(self.reference_track_id)?;
        writer.write_u64::<BigEndian>(self.ntp_timestamp)?;

        if self.version == 0 {
            writer.write_u32::<BigEndian>(self.media_time as u32)?;
        } else {
            writer.write_u64::<BigEndian>(self.media_time)?;
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
    fn test_prft() {
        let src_box = PrftBox {
            version: 0,
            flags: 0,
            media_time: 123,
            ntp_timestamp: 321,
            reference_track_id: 1,
        };
        let mut buf = Vec::new();
        src_box.write_box(&mut buf).unwrap();
        assert_eq!(buf.len(), src_box.box_size() as usize);

        let mut reader = Cursor::new(&buf);
        let header = BoxHeader::read(&mut reader).unwrap();
        assert_eq!(header.name, BoxType::PrftBox);
        assert_eq!(src_box.box_size(), header.size);

        let dst_box = PrftBox::read_box(&mut reader, header.size).unwrap();
        assert_eq!(src_box, dst_box);
    }
}
