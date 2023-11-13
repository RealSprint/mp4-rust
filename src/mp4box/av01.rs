use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use serde::Serialize;
use std::io::{Read, Seek, Write};

use crate::{colr::ColrBox, mp4box::*, pasp::PaspBox};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Av01Box {
    pub data_reference_index: u16,
    pub width: u16,
    pub height: u16,

    #[serde(with = "value_u32")]
    pub horizresolution: FixedPointU16,

    #[serde(with = "value_u32")]
    pub vertresolution: FixedPointU16,
    pub frame_count: u16,
    pub depth: u16,
    pub av1c: Av1CBox,
    pub colr: Option<ColrBox>,
    pub pasp: Option<PaspBox>,
}

impl Default for Av01Box {
    fn default() -> Self {
        Av01Box {
            data_reference_index: 0,
            width: 0,
            height: 0,
            horizresolution: FixedPointU16::new(0x48),
            vertresolution: FixedPointU16::new(0x48),
            frame_count: 1,
            depth: 0x0018,
            av1c: Av1CBox::default(),
            colr: None,
            pasp: None,
        }
    }
}

impl Av01Box {
    pub fn new(config: &Av1Config) -> Self {
        Av01Box {
            data_reference_index: 1,
            width: config.width,
            height: config.height,
            horizresolution: FixedPointU16::new(0x48),
            vertresolution: FixedPointU16::new(0x48),
            frame_count: 1,
            depth: 0x0018,
            av1c: Av1CBox::new(config),
            colr: config.color.as_ref().map(|color| ColrBox {
                color_config: colr::Color::Nclx(color.clone()),
            }),
            pasp: config
                .aspect_ratio
                .as_ref()
                .map(|(numerator, denumerator)| PaspBox {
                    numerator: *numerator,
                    denumerator: *denumerator,
                }),
        }
    }

    pub fn get_type(&self) -> BoxType {
        BoxType::Av01Box
    }

    pub fn get_size(&self) -> u64 {
        let mut size = HEADER_SIZE + 8 + 70 + self.av1c.box_size();

        if let Some(colr) = &self.colr {
            size += colr.box_size();
        }

        if let Some(pasp) = &self.pasp {
            size += pasp.box_size();
        }

        size
    }
}

impl Mp4Box for Av01Box {
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
            "data_reference_index={} width={} height={} frame_count={}",
            self.data_reference_index, self.width, self.height, self.frame_count
        );
        Ok(s)
    }
}

impl<R: Read + Seek> ReadBox<&mut R> for Av01Box {
    fn read_box(reader: &mut R, size: u64) -> Result<Self> {
        let start = box_start(reader)?;

        reader.read_u32::<BigEndian>()?; // reserved
        reader.read_u16::<BigEndian>()?; // reserved
        let data_reference_index = reader.read_u16::<BigEndian>()?;

        reader.read_u32::<BigEndian>()?; // pre-defined, reserved
        reader.read_u64::<BigEndian>()?; // pre-defined
        reader.read_u32::<BigEndian>()?; // pre-defined
        let width = reader.read_u16::<BigEndian>()?;
        let height = reader.read_u16::<BigEndian>()?;
        let horizresolution = FixedPointU16::new_raw(reader.read_u32::<BigEndian>()?);
        let vertresolution = FixedPointU16::new_raw(reader.read_u32::<BigEndian>()?);
        reader.read_u32::<BigEndian>()?; // reserved
        let frame_count = reader.read_u16::<BigEndian>()?;
        skip_bytes(reader, 32)?; // compressorname
        let depth = reader.read_u16::<BigEndian>()?;
        reader.read_i16::<BigEndian>()?; // pre-defined

        //
        let mut av1c = None;
        let mut colr = None;
        let mut pasp = None;

        let mut current = reader.stream_position()?;
        let end = start + size;
        while current < end {
            // Get box header.
            let header = BoxHeader::read(reader)?;
            let BoxHeader { name, size: s } = header;
            if s > size {
                return Err(Error::InvalidData(
                    "av01 box contains a box with a larger size than it",
                ));
            }

            match name {
                BoxType::Av1CBox => {
                    av1c = Some(Av1CBox::read_box(reader, s)?);
                }
                BoxType::ColrBox => {
                    colr = Some(ColrBox::read_box(reader, s)?);
                }
                BoxType::PaspBox => {
                    pasp = Some(PaspBox::read_box(reader, s)?);
                }
                _ => {
                    // XXX warn!()
                    skip_box(reader, s)?;
                }
            }
            current = reader.stream_position()?;
        }

        skip_bytes_to(reader, start + size)?;

        let Some(av1c) = av1c else {
            return Err(Error::InvalidData("av1c not found"));
        };

        Ok(Av01Box {
            data_reference_index,
            width,
            height,
            horizresolution,
            vertresolution,
            frame_count,
            depth,
            av1c,
            colr,
            pasp,
        })
    }
}

impl<W: Write> WriteBox<&mut W> for Av01Box {
    fn write_box(&self, writer: &mut W) -> Result<u64> {
        let size = self.box_size();
        BoxHeader::new(self.box_type(), size).write(writer)?;

        writer.write_u32::<BigEndian>(0)?; // reserved
        writer.write_u16::<BigEndian>(0)?; // reserved
        writer.write_u16::<BigEndian>(self.data_reference_index)?;

        writer.write_u32::<BigEndian>(0)?; // pre-defined, reserved
        writer.write_u64::<BigEndian>(0)?; // pre-defined
        writer.write_u32::<BigEndian>(0)?; // pre-defined
        writer.write_u16::<BigEndian>(self.width)?;
        writer.write_u16::<BigEndian>(self.height)?;
        writer.write_u32::<BigEndian>(self.horizresolution.raw_value())?;
        writer.write_u32::<BigEndian>(self.vertresolution.raw_value())?;
        writer.write_u32::<BigEndian>(0)?; // reserved
        writer.write_u16::<BigEndian>(self.frame_count)?;
        // skip compressorname
        write_zeros(writer, 32)?;
        writer.write_u16::<BigEndian>(self.depth)?;
        writer.write_i16::<BigEndian>(-1)?; // pre-defined

        self.av1c.write_box(writer)?;

        if let Some(colr) = &self.colr {
            colr.write_box(writer)?;
        }

        if let Some(pasp) = &self.pasp {
            pasp.write_box(writer)?;
        }

        Ok(size)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct Av1CBox {
    pub tier: u8,
    pub profile: u8,
    pub level_idx: u8,
    pub bit_depth: u8,
    pub monochrome: bool,
    pub subsampling_x: u8,
    pub subsampling_y: u8,
    pub chroma_sample_position: u8,
    pub initial_presentation_delay_minus_one: Option<u8>,
    pub sequence_header: Vec<u8>,
}

impl Av1CBox {
    pub fn new(config: &Av1Config) -> Self {
        Self {
            tier: config.tier,
            profile: config.profile,
            level_idx: config.level_idx,
            bit_depth: config.bit_depth,
            monochrome: config.monochrome,
            subsampling_x: config.subsampling_x,
            subsampling_y: config.subsampling_y,
            chroma_sample_position: config.chroma_sample_position,
            sequence_header: config.sequence_header.clone(),
            initial_presentation_delay_minus_one: config.initial_presentation_delay_minus_one,
        }
    }
}

impl Mp4Box for Av1CBox {
    fn box_type(&self) -> BoxType {
        BoxType::Av1CBox
    }

    fn box_size(&self) -> u64 {
        let mut size = HEADER_SIZE + 4;
        size += self.sequence_header.len() as u64;
        size
    }

    fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string(&self).unwrap())
    }

    fn summary(&self) -> Result<String> {
        let s = format!("profile={}", self.profile);
        Ok(s)
    }
}

impl<R: Read + Seek> ReadBox<&mut R> for Av1CBox {
    fn read_box(reader: &mut R, size: u64) -> Result<Self> {
        let start = box_start(reader)?;

        let mut info = [0; 4];
        reader.read_exact(&mut info)?;
        let profile = info[1] >> 5;
        let level_idx = info[1] & 0x1f;
        let tier = info[2] >> 7;
        let high_bit_depth = (info[2] >> 6) & 1 == 1;
        let bit_depth_twelve = (info[2] >> 5) & 1 == 1;
        let monochrome = (info[2] >> 4) & 1 == 1;
        let subsampling_x = (info[2] >> 3) & 1;
        let subsampling_y = (info[2] >> 2) & 1;
        let chroma_sample_position = info[2] & 0x3;

        let initial_presentation_delay_present = ((info[3] >> 4) & 1) == 1;
        let initial_presentation_delay_minus_one = info[3] & 0xf;

        let position = reader.stream_position()?;
        let sequence_header_length = start + size - position;
        let mut sequence_header = vec![0; sequence_header_length as usize];
        reader.read_exact(&mut sequence_header)?;

        skip_bytes_to(reader, start + size)?;

        let bit_depth = match (high_bit_depth, bit_depth_twelve) {
            (true, true) => 12,
            (true, false) => 10,
            (false, _) => 8,
        };

        Ok(Av1CBox {
            sequence_header,
            profile,
            level_idx,
            tier,
            bit_depth,
            monochrome,
            subsampling_x,
            subsampling_y,
            chroma_sample_position,
            initial_presentation_delay_minus_one: initial_presentation_delay_present
                .then_some(initial_presentation_delay_minus_one),
        })
    }
}

impl<W: Write> WriteBox<&mut W> for Av1CBox {
    fn write_box(&self, writer: &mut W) -> Result<u64> {
        let size = self.box_size();
        BoxHeader::new(self.box_type(), size).write(writer)?;

        writer.write_u8(129)?; // marker + version
        writer.write_u8(self.profile << 5 | self.level_idx)?;
        writer.write_u8(
            self.tier << 7
                | u8::from(self.bit_depth > 8) << 6
                | u8::from(self.bit_depth == 12) << 5
                | u8::from(self.monochrome) << 4
                | self.subsampling_x << 3
                | self.subsampling_y << 2
                | self.chroma_sample_position,
        )?;

        // TODO: write initial presentation delay
        writer.write_u8(0)?;

        writer.write_all(&self.sequence_header)?;
        Ok(size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mp4box::BoxHeader;
    use std::io::Cursor;

    #[test]
    fn test_av01() {
        let src_box = Av01Box {
            data_reference_index: 1,
            width: 320,
            height: 240,
            horizresolution: FixedPointU16::new(0x48),
            vertresolution: FixedPointU16::new(0x48),
            frame_count: 1,
            depth: 24,
            av1c: Av1CBox {
                tier: 0,
                profile: 0,
                level_idx: 8,
                bit_depth: 8,
                monochrome: false,
                subsampling_x: 1,
                subsampling_y: 1,
                chroma_sample_position: 0,
                initial_presentation_delay_minus_one: None,
                sequence_header: vec![10, 11, 0, 0, 0, 66, 167, 191, 230, 46, 223, 200, 66],
            },
            colr: Some(ColrBox {
                color_config: colr::Color::Nclx(ColorConfig {
                    color_primaries: 9,
                    transfer_characteristics: 16,
                    matrix_coefficients: 9,
                    full_range: false,
                }),
            }),
            pasp: Some(PaspBox {
                numerator: 16,
                denumerator: 9,
            }),
        };
        let mut buf = Vec::new();
        src_box.write_box(&mut buf).unwrap();
        assert_eq!(buf.len(), src_box.box_size() as usize);

        let mut reader = Cursor::new(&buf);
        let header = BoxHeader::read(&mut reader).unwrap();
        assert_eq!(header.name, BoxType::Av01Box);
        assert_eq!(src_box.box_size(), header.size);

        let dst_box = Av01Box::read_box(&mut reader, header.size).unwrap();
        assert_eq!(src_box, dst_box);
    }
}
