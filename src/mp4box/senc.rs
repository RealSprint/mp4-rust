use std::io::{Read, Seek, Write};

use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use serde::Serialize;

use super::{
    box_start, read_box_header_ext, skip_bytes_to, write_box_header_ext, BoxHeader, BoxType, Error,
    Mp4Box, ReadBox, Result, WriteBox, HEADER_EXT_SIZE, HEADER_SIZE,
};

// ISO 23001-7:2023 - 7.2.1 Sample Encryption Box
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct SencBox {
    version: u8,
    use_sub_samples: bool,

    sample_count: u32,

    ivs: Vec<SencData>,
}

impl SencBox {
    pub fn get_type(&self) -> BoxType {
        BoxType::SencBox
    }

    pub fn get_size(&self) -> u64 {
        let iv_size = self
            .ivs
            .iter()
            .map(|iv| {
                16 + if self.use_sub_samples {
                    2 + iv.sub_samples.len() as u64 * 6
                } else {
                    0
                }
            })
            .sum::<u64>();

        HEADER_SIZE + HEADER_EXT_SIZE + 4 + iv_size
    }
}

impl Mp4Box for SencBox {
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
        Ok(String::new())
    }
}

impl<R: Read + Seek> ReadBox<&mut R> for SencBox {
    fn read_box(reader: &mut R, size: u64) -> Result<Self> {
        let start = box_start(reader)?;

        let (version, flags) = read_box_header_ext(reader)?;
        let use_subsamples = (flags & 0x2) == 0x2;

        if use_subsamples && version != 0 {
            return Err(Error::InvalidData(
                "use_sub_samples is only valid when version is 0",
            ));
        }

        let senc = match version {
            0 => read_version0(reader, size, use_subsamples),
            1 => read_version1(reader, size),
            2 => read_version2(reader, size),
            _ => Err(Error::InvalidData("Invalid version")),
        };

        skip_bytes_to(reader, start + size)?;

        senc
    }
}

fn read_version0<R: Read + Seek>(
    reader: &mut R,
    _size: u64,
    use_sub_samples: bool,
) -> Result<SencBox> {
    let sample_count = reader.read_u32::<BigEndian>()?;

    println!("sample_count: {}", sample_count);

    let mut ivs = Vec::new();
    for _ in 0..sample_count {
        // TODO: Is this really always 16, or can it be 8?
        let mut iv = [0; 16];
        reader.read_exact(&mut iv)?;

        let mut sub_samples = Vec::new();
        if use_sub_samples {
            let sub_sample_count = reader.read_u16::<BigEndian>()?;

            for _ in 0..sub_sample_count {
                let clear_data = reader.read_u16::<BigEndian>()?;
                let encrypted_data = reader.read_u32::<BigEndian>()?;

                sub_samples.push(SubSample {
                    clear_data,
                    encrypted_data,
                });
            }
        }
        ivs.push(SencData { iv, sub_samples });
    }

    Ok(SencBox {
        sample_count,
        version: 0,
        ivs,
        use_sub_samples,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
struct SencData {
    iv: [u8; 16],
    sub_samples: Vec<SubSample>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
struct SubSample {
    clear_data: u16,
    encrypted_data: u32,
}

fn read_version1<R: Read + Seek>(_reader: &mut R, _size: u64) -> Result<SencBox> {
    Err(Error::NotImplemented)
}

fn read_version2<R: Read + Seek>(_reader: &mut R, _size: u64) -> Result<SencBox> {
    Err(Error::NotImplemented)
}

impl<W: Write> WriteBox<&mut W> for SencBox {
    fn write_box(&self, writer: &mut W) -> Result<u64> {
        let size = self.box_size();

        BoxHeader::new(self.box_type(), size).write(writer)?;
        write_box_header_ext(
            writer,
            self.version,
            if self.use_sub_samples { 0x2 } else { 0 },
        )?;

        match self.version {
            0 => write_version0(writer, self),
            1 => write_version1(writer, self),
            2 => write_version2(writer, self),
            _ => Err(Error::InvalidData("Invalid version")),
        }?;

        Ok(size)
    }
}

fn write_version0<W: Write>(writer: &mut W, senc: &SencBox) -> Result<()> {
    writer.write_u32::<BigEndian>(senc.sample_count)?;
    for iv in &senc.ivs {
        writer.write_all(&iv.iv)?;

        if senc.use_sub_samples {
            writer.write_u16::<BigEndian>(iv.sub_samples.len() as u16)?;
            for sub_sample in &iv.sub_samples {
                writer.write_u16::<BigEndian>(sub_sample.clear_data)?;
                writer.write_u32::<BigEndian>(sub_sample.encrypted_data)?;
            }
        }
    }

    Ok(())
}
fn write_version1<W: Write>(_writer: &mut W, _senc: &SencBox) -> Result<()> {
    Err(Error::NotImplemented)
}
fn write_version2<W: Write>(_writer: &mut W, _senc: &SencBox) -> Result<()> {
    Err(Error::NotImplemented)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mp4box::BoxHeader;
    use std::io::Cursor;

    #[test]
    fn test_senc_v1_with_sub_samples() {
        let src_box = SencBox {
            sample_count: 2,
            version: 0,
            use_sub_samples: true,
            ivs: vec![
                SencData {
                    iv: [
                        0xe8, 0x6b, 0x4c, 0xa8, 0xae, 0x2c, 0x3f, 0xbd, //
                        0x88, 0x07, 0x41, 0x4f, 0x2a, 0xdf, 0x5a, 0xcc, //
                    ],
                    sub_samples: vec![SubSample {
                        clear_data: 773,
                        encrypted_data: 19472,
                    }],
                },
                SencData {
                    iv: [
                        0xe8, 0x6b, 0x4c, 0xa8, 0xae, 0x2c, 0x3f, 0xbd, //
                        0x88, 0x07, 0x41, 0x4f, 0x2a, 0xdf, 0x5f, 0x8d, //
                    ],
                    sub_samples: vec![SubSample {
                        clear_data: 19,
                        encrypted_data: 5632,
                    }],
                },
            ],
        };

        let mut buf = Vec::new();
        src_box.write_box(&mut buf).unwrap();
        assert_eq!(buf.len(), src_box.box_size() as usize);

        let expected = vec![
            0x00, 0x00, 0x00, 0x40, b's', b'e', b'n', b'c', // header
            0x00, 0x00, 0x00, 0x02, // header ext
            0x00, 0x00, 0x00, 0x02, // sample_count
            0xe8, 0x6b, 0x4c, 0xa8, 0xae, 0x2c, 0x3f, 0xbd, // IV
            0x88, 0x07, 0x41, 0x4f, 0x2a, 0xdf, 0x5a, 0xcc, // IV
            0x00, 0x01, // sub_sample_count
            0x03, 0x05, 0x00, 0x00, 0x4c, 0x10, // sub_sample
            0xe8, 0x6b, 0x4c, 0xa8, 0xae, 0x2c, 0x3f, 0xbd, // IV
            0x88, 0x07, 0x41, 0x4f, 0x2a, 0xdf, 0x5f, 0x8d, // IV
            0x00, 0x01, // sub_sample_count
            0x00, 0x13, 0x00, 0x00, 0x16, 0x00, // sub_sample
        ];

        assert_eq!(buf, expected);

        let mut reader = Cursor::new(&buf);
        let header = BoxHeader::read(&mut reader).unwrap();
        assert_eq!(header.name, BoxType::SencBox);
        assert_eq!(src_box.box_size(), header.size);

        let dst_box = SencBox::read_box(&mut reader, header.size).unwrap();
        assert_eq!(src_box, dst_box);
    }
}
