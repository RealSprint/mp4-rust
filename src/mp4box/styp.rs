use serde::Serialize;
use std::io::{Read, Seek, Write};

use crate::mp4box::*;

use super::psuedo_boxes::general_type_box::{GeneralTypeBox, GeneralTypeBoxType};

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct StypBox(pub GeneralTypeBox);

impl StypBox {
    pub fn get_type(&self) -> BoxType {
        BoxType::StypBox
    }

    pub fn get_size(&self) -> u64 {
        self.0.get_size()
    }
}

impl Mp4Box for StypBox {
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
        self.0.summary()
    }
}

impl<R: Read + Seek> ReadBox<&mut R> for StypBox {
    fn read_box(reader: &mut R, size: u64, context: &mut Mp4Context) -> Result<Self> {
        Ok(Self(GeneralTypeBox::read_box(reader, size, context)?))
    }
}

impl<W: Write> WriteBox<&mut W> for StypBox {
    fn write_box(&self, writer: &mut W) -> Result<u64> {
        self.0.write_box(writer, GeneralTypeBoxType::StypBox)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mp4box::BoxHeader;
    use std::io::Cursor;

    #[test]
    fn test_styp() {
        let src_box = StypBox(GeneralTypeBox {
            major_brand: str::parse("isom").unwrap(),
            minor_version: 0,
            compatible_brands: vec![
                str::parse("isom").unwrap(),
                str::parse("iso2").unwrap(),
                str::parse("avc1").unwrap(),
                str::parse("mp41").unwrap(),
            ],
        });
        let mut buf = Vec::new();
        src_box.write_box(&mut buf).unwrap();
        assert_eq!(buf.len(), src_box.box_size() as usize);

        let mut reader = Cursor::new(&buf);
        let header = BoxHeader::read(&mut reader).unwrap();
        assert_eq!(header.name, BoxType::StypBox);
        assert_eq!(src_box.box_size(), header.size);

        let dst_box =
            StypBox::read_box(&mut reader, header.size, &mut Mp4Context::default()).unwrap();
        assert_eq!(src_box, dst_box);
    }
}
