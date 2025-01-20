use std::io::{Read, Seek, Write};

use serde::Serialize;

use crate::skip_box;

use super::{
    box_start, frma::FrmaBox, schi::SchiBox, schm::SchmBox, skip_bytes_to, BoxHeader, BoxType,
    Error, Mp4Box, ReadBox, Result, WriteBox,
};

// ISO 14496-12:2022 - 8.12.2 Protection Scheme Information Box
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct SinfBox {
    frma: FrmaBox,
    schi: Option<SchiBox>,
    schm: Option<SchmBox>,
}

impl SinfBox {
    pub fn get_type(&self) -> BoxType {
        BoxType::SinfBox
    }

    pub fn get_size(&self) -> u64 {
        let schi_size = &self.schi.as_ref().map_or(0, |schi| schi.box_size());
        let schm_size = &self.schm.as_ref().map_or(0, |schm| schm.box_size());

        8 + self.frma.box_size() + schi_size + schm_size
    }
}

impl Mp4Box for SinfBox {
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

impl<R: Read + Seek> ReadBox<&mut R> for SinfBox {
    fn read_box(reader: &mut R, size: u64) -> Result<Self> {
        let start = box_start(reader)?;

        let mut current = reader.stream_position()?;
        let end = start + size;

        let mut frma = None;
        let mut schi = None;
        let mut schm = None;

        while current < end {
            let header = BoxHeader::read(reader)?;
            let BoxHeader { name, size: s } = header;
            if s > size {
                return Err(Error::InvalidData(
                    "sinf box contains a box with a larger size than it",
                ));
            }

            match name {
                BoxType::FrmaBox => {
                    frma = Some(FrmaBox::read_box(reader, s)?);
                }
                BoxType::SchiBox => {
                    schi = Some(SchiBox::read_box(reader, s)?);
                }
                BoxType::SchmBox => {
                    schm = Some(SchmBox::read_box(reader, s)?);
                }
                _ => {
                    // TODO: Log box type
                    skip_box(reader, s)?;
                }
            }
            current = reader.stream_position()?;
        }

        skip_bytes_to(reader, start + size)?;

        Ok(SinfBox {
            frma: frma.ok_or(Error::BoxNotFound(BoxType::FrmaBox))?,
            schi,
            schm,
        })
    }
}

impl<W: Write> WriteBox<&mut W> for SinfBox {
    fn write_box(&self, writer: &mut W) -> Result<u64> {
        let size = self.box_size();
        BoxHeader::new(self.box_type(), size).write(writer)?;

        self.frma.write_box(writer)?;

        if let Some(ref schi) = self.schi {
            schi.write_box(writer)?;
        }

        if let Some(ref schm) = self.schm {
            schm.write_box(writer)?;
        }

        Ok(size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{mp4box::BoxHeader, tenc::TencBox, FourCC};
    use std::io::Cursor;

    #[test]
    fn test_sinf_only_frma() {
        let src_box = SinfBox {
            frma: FrmaBox {
                data_format: crate::FourCC { value: *b"avc1" },
            },
            schi: None,
            schm: None,
        };

        let mut buf = Vec::new();
        src_box.write_box(&mut buf).unwrap();
        assert_eq!(buf.len(), src_box.box_size() as usize);

        let expected: Vec<u8> = vec![
            0x00, 0x00, 0x00, 0x14, b's', b'i', b'n', b'f', //
            0x00, 0x00, 0x00, 0x0c, b'f', b'r', b'm', b'a', //
            b'a', b'v', b'c', b'1', //
        ];
        assert_eq!(buf, expected);

        let mut reader = Cursor::new(&buf);
        let header = BoxHeader::read(&mut reader).unwrap();
        assert_eq!(header.name, BoxType::SinfBox);
        assert_eq!(src_box.box_size(), header.size);

        let dst_box = SinfBox::read_box(&mut reader, header.size).unwrap();
        assert_eq!(src_box, dst_box);
    }

    #[test]
    fn test_sinf_schi() {
        let src_box = SinfBox {
            frma: FrmaBox {
                data_format: FourCC { value: *b"avc1" },
            },
            schi: Some(SchiBox {
                tenc: TencBox::new_unprotected(),
            }),
            schm: None,
        };

        let mut buf = Vec::new();
        src_box.write_box(&mut buf).unwrap();
        assert_eq!(buf.len(), src_box.box_size() as usize);

        let expected: Vec<u8> = vec![
            0x00, 0x00, 0x00, 0x3c, b's', b'i', b'n', b'f', //
            // frma
            0x00, 0x00, 0x00, 0x0c, b'f', b'r', b'm', b'a', //
            b'a', b'v', b'c', b'1', //
            // schi
            0x00, 0x00, 0x00, 0x28, b's', b'c', b'h', b'i', //
            0x00, 0x00, 0x00, 0x20, b't', b'e', b'n', b'c', //
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
        ];
        assert_eq!(buf, expected);

        let mut reader = Cursor::new(&buf);
        let header = BoxHeader::read(&mut reader).unwrap();
        assert_eq!(header.name, BoxType::SinfBox);
        assert_eq!(src_box.box_size(), header.size);

        let dst_box = SinfBox::read_box(&mut reader, header.size).unwrap();
        assert_eq!(src_box, dst_box);
    }

    #[test]
    fn test_sinf_schm() {
        let src_box = SinfBox {
            frma: FrmaBox {
                data_format: FourCC { value: *b"avc1" },
            },
            schi: None,
            schm: Some(SchmBox {
                version: 0,
                flags: 0,
                scheme_type: FourCC { value: *b"cenc" },
                scheme_uri: None,
                scheme_version: 0x10000,
            }),
        };

        let mut buf = Vec::new();
        src_box.write_box(&mut buf).unwrap();
        assert_eq!(buf.len(), src_box.box_size() as usize);

        let expected: Vec<u8> = vec![
            0x00, 0x00, 0x00, 0x28, b's', b'i', b'n', b'f', //
            // frma
            0x00, 0x00, 0x00, 0x0c, b'f', b'r', b'm', b'a', //
            b'a', b'v', b'c', b'1', //
            // schm
            0x00, 0x00, 0x00, 0x14, b's', b'c', b'h', b'm', //
            0x00, 0x00, 0x00, 0x00, b'c', b'e', b'n', b'c', //
            0x00, 0x01, 0x00, 0x00, //
        ];
        assert_eq!(buf, expected);

        let mut reader = Cursor::new(&buf);
        let header = BoxHeader::read(&mut reader).unwrap();
        assert_eq!(header.name, BoxType::SinfBox);
        assert_eq!(src_box.box_size(), header.size);

        let dst_box = SinfBox::read_box(&mut reader, header.size).unwrap();
        assert_eq!(src_box, dst_box);
    }

    #[test]
    fn test_sinf_schi_schm() {
        let src_box = SinfBox {
            frma: FrmaBox {
                data_format: FourCC { value: *b"avc1" },
            },
            schi: Some(SchiBox {
                tenc: TencBox::new_unprotected(),
            }),
            schm: Some(SchmBox {
                version: 0,
                flags: 0,
                scheme_type: FourCC { value: *b"cenc" },
                scheme_uri: None,
                scheme_version: 0x10000,
            }),
        };

        let mut buf = Vec::new();
        src_box.write_box(&mut buf).unwrap();
        assert_eq!(buf.len(), src_box.box_size() as usize);

        let expected: Vec<u8> = vec![
            0x00, 0x00, 0x00, 0x50, b's', b'i', b'n', b'f', //
            // frma
            0x00, 0x00, 0x00, 0x0c, b'f', b'r', b'm', b'a', //
            b'a', b'v', b'c', b'1', //
            // schi
            0x00, 0x00, 0x00, 0x28, b's', b'c', b'h', b'i', //
            0x00, 0x00, 0x00, 0x20, b't', b'e', b'n', b'c', //
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
            // schm
            0x00, 0x00, 0x00, 0x14, b's', b'c', b'h', b'm', //
            0x00, 0x00, 0x00, 0x00, b'c', b'e', b'n', b'c', //
            0x00, 0x01, 0x00, 0x00, //
        ];
        assert_eq!(buf, expected);

        let mut reader = Cursor::new(&buf);
        let header = BoxHeader::read(&mut reader).unwrap();
        assert_eq!(header.name, BoxType::SinfBox);
        assert_eq!(src_box.box_size(), header.size);

        let dst_box = SinfBox::read_box(&mut reader, header.size).unwrap();
        assert_eq!(src_box, dst_box);
    }
}
