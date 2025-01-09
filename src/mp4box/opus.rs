use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use serde::Serialize;
use std::io::{Read, Seek, Write};

use crate::mp4box::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OpusBox {
    pub data_reference_index: u16,
    pub samplesize: u16,

    #[serde(with = "value_u32")]
    pub samplerate: FixedPointU16,
    pub dops: DopsBox,
}

impl OpusBox {
    pub fn new(config: &OpusConfig) -> Self {
        Self {
            data_reference_index: 1,
            samplesize: 16,
            samplerate: FixedPointU16::new(config.sample_rate as u16),
            dops: DopsBox {
                version: 0,
                pre_skip: config.pre_skip,
                input_sample_rate: config.sample_rate,
                output_gain: config.output_gain,
                channel_mapping_family: config.channel_mapping_family.clone(),
            },
        }
    }

    pub fn get_type(&self) -> BoxType {
        BoxType::OpusBox
    }

    pub fn get_size(&self) -> u64 {
        let mut size = HEADER_SIZE + 8 + 20;
        size += self.dops.box_size();

        size
    }
}

impl Mp4Box for OpusBox {
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
            "channel_count={} sample_size={} sample_rate={}",
            self.dops.channel_mapping_family.get_channel_count(),
            self.samplesize,
            self.samplerate.value()
        );
        Ok(s)
    }
}

impl<R: Read + Seek> ReadBox<&mut R> for OpusBox {
    fn read_box(reader: &mut R, size: u64) -> Result<Self> {
        let start = box_start(reader)?;

        reader.read_u32::<BigEndian>()?; // reserved
        reader.read_u16::<BigEndian>()?; // reserved
        let data_reference_index = reader.read_u16::<BigEndian>()?;
        let version = reader.read_u16::<BigEndian>()?;
        reader.read_u16::<BigEndian>()?; // reserved
        reader.read_u32::<BigEndian>()?; // reserved
        let _channelcount = reader.read_u16::<BigEndian>()?;
        let samplesize = reader.read_u16::<BigEndian>()?;
        reader.read_u32::<BigEndian>()?; // pre-defined, reserved
        let samplerate = FixedPointU16::new_raw(reader.read_u32::<BigEndian>()?);

        if version == 1 {
            // Skip QTFF
            reader.read_u64::<BigEndian>()?;
            reader.read_u64::<BigEndian>()?;
        }

        let header = BoxHeader::read(reader)?;
        let BoxHeader { name, size: s } = header;
        if s > size {
            return Err(Error::InvalidData(
                "opus box contains a box with a larger size than it",
            ));
        }
        if name == BoxType::DopsBox {
            let dops = DopsBox::read_box(reader, s)?;

            skip_bytes_to(reader, start + size)?;

            Ok(OpusBox {
                data_reference_index,
                samplesize,
                samplerate,
                dops,
            })
        } else {
            Err(Error::InvalidData("dops not found"))
        }
    }
}

impl<W: Write> WriteBox<&mut W> for OpusBox {
    fn write_box(&self, writer: &mut W) -> Result<u64> {
        let size = self.box_size();
        BoxHeader::new(self.box_type(), size).write(writer)?;

        writer.write_u32::<BigEndian>(0)?; // reserved
        writer.write_u16::<BigEndian>(0)?; // reserved
        writer.write_u16::<BigEndian>(self.data_reference_index)?;
        writer.write_u64::<BigEndian>(0)?; // reserved
        writer
            .write_u16::<BigEndian>(self.dops.channel_mapping_family.get_channel_count() as u16)?;
        writer.write_u16::<BigEndian>(self.samplesize)?;
        writer.write_u32::<BigEndian>(0)?; // reserved
        writer.write_u32::<BigEndian>(self.samplerate.raw_value())?;

        self.dops.write_box(writer)?;

        Ok(size)
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize)]
pub struct ChannelMapping {
    pub stream_count: u8,
    pub coupled_count: u8,
    pub channel_mapping: Vec<u8>,
}

impl ChannelMapping {
    fn read<R: Read + Seek>(reader: &mut R, channel_count: u8) -> Result<Self> {
        let stream_count = reader.read_u8()?;
        let coupled_count = reader.read_u8()?;
        let mut channel_mapping = vec![0u8; channel_count as usize];
        reader.read_exact(&mut channel_mapping)?;
        Ok(Self {
            stream_count,
            coupled_count,
            channel_mapping,
        })
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<usize> {
        writer.write_u8(self.stream_count)?;
        writer.write_u8(self.coupled_count)?;
        writer.write_all(&self.channel_mapping)?;
        Ok(1 + 1 + self.channel_mapping.len())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum ChannelMappingFamily {
    Family0 { stereo: bool },
    Family1(ChannelMapping),
    Unknown(ChannelMapping),
}

impl ChannelMappingFamily {
    fn byte_size(&self) -> usize {
        match self {
            ChannelMappingFamily::Family0 { .. } => 2,
            ChannelMappingFamily::Family1(mapping) => 4 + mapping.channel_mapping.len(),
            ChannelMappingFamily::Unknown(mapping) => 4 + mapping.channel_mapping.len(),
        }
    }

    pub fn get_channel_count(&self) -> u8 {
        match self {
            ChannelMappingFamily::Family0 { stereo } => {
                if *stereo {
                    2
                } else {
                    1
                }
            }
            ChannelMappingFamily::Family1(mapping) => mapping.channel_mapping.len() as u8,
            ChannelMappingFamily::Unknown(mapping) => mapping.channel_mapping.len() as u8,
        }
    }

    fn get_channel_family(&self) -> u8 {
        match self {
            ChannelMappingFamily::Family0 { .. } => 0,
            ChannelMappingFamily::Family1(_) => 1,
            ChannelMappingFamily::Unknown(_) => 255,
        }
    }

    fn read<R: Seek + Read>(reader: &mut R, channel_count: u8) -> Result<Self> {
        let family: u8 = reader.read_u8()?;
        Ok(match family {
            0 => Self::Family0 {
                stereo: channel_count == 2,
            },
            1 => Self::Family1(ChannelMapping::read(reader, channel_count)?),
            _ => Self::Unknown(ChannelMapping::read(reader, channel_count)?),
        })
    }

    fn write<W: Write>(&self, writer: &mut W) -> Result<usize> {
        let mut count = 0;

        writer.write_u8(self.get_channel_family())?;
        count += 1;
        count += match self {
            ChannelMappingFamily::Family0 { .. } => 0,
            ChannelMappingFamily::Family1(mapping) => {
                debug_assert!(
                    mapping.channel_mapping.len() <= 8,
                    "Opus Family1 cannot have more than 8 output channels"
                );

                mapping.write(writer)?
            }
            ChannelMappingFamily::Unknown(mapping) => {
                debug_assert!(
                    mapping.channel_mapping.len() <= 255,
                    "Opus Unknown Family cannot have more than 255 output channels"
                );
                mapping.write(writer)?
            }
        };
        Ok(count)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DopsBox {
    pub version: u8,
    pub pre_skip: u16,
    pub input_sample_rate: u32,
    pub output_gain: i16,
    pub channel_mapping_family: ChannelMappingFamily,
}

impl Mp4Box for DopsBox {
    fn box_type(&self) -> BoxType {
        BoxType::DopsBox
    }

    fn box_size(&self) -> u64 {
        HEADER_SIZE + 1 + 2 + 4 + 2 + self.channel_mapping_family.byte_size() as u64
    }

    fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string(&self).unwrap())
    }

    fn summary(&self) -> Result<String> {
        Ok(String::new())
    }
}

impl<R: Read + Seek> ReadBox<&mut R> for DopsBox {
    fn read_box(reader: &mut R, _size: u64) -> Result<Self> {
        let _start = box_start(reader)?;
        let version = reader.read_u8()?;
        let output_channel_count = reader.read_u8()?;
        let pre_skip = reader.read_u16::<BigEndian>()?;
        let input_sample_rate = reader.read_u32::<BigEndian>()?;
        let output_gain = reader.read_i16::<BigEndian>()?;
        let channel_mapping_family = ChannelMappingFamily::read(reader, output_channel_count)?;

        Ok(DopsBox {
            version,
            output_gain,
            pre_skip,
            input_sample_rate,
            channel_mapping_family,
        })
    }
}

impl<W: Write> WriteBox<&mut W> for DopsBox {
    fn write_box(&self, writer: &mut W) -> Result<u64> {
        let size = self.box_size();
        BoxHeader::new(self.box_type(), size).write(writer)?;

        writer.write_u8(self.version)?;
        writer.write_u8(self.channel_mapping_family.get_channel_count())?;
        writer.write_u16::<BigEndian>(self.pre_skip)?;
        writer.write_u32::<BigEndian>(self.input_sample_rate)?;
        writer.write_i16::<BigEndian>(self.output_gain)?;

        self.channel_mapping_family.write(writer)?;

        Ok(size)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct SLConfigDescriptor {}

impl SLConfigDescriptor {
    pub fn new() -> Self {
        SLConfigDescriptor {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mp4box::BoxHeader;
    use std::io::Cursor;

    #[test]
    fn test_mp4a() {
        let src_box = OpusBox {
            data_reference_index: 1,
            samplesize: 16,
            samplerate: FixedPointU16::new(48000),
            dops: DopsBox {
                version: 0,
                pre_skip: 1,
                input_sample_rate: 2,
                output_gain: 3,
                channel_mapping_family: ChannelMappingFamily::Family0 { stereo: true },
            },
        };
        let mut buf = Vec::new();
        src_box.write_box(&mut buf).unwrap();
        assert_eq!(buf.len(), src_box.box_size() as usize);

        let mut reader = Cursor::new(&buf);
        let header = BoxHeader::read(&mut reader).unwrap();
        assert_eq!(header.name, BoxType::OpusBox);
        assert_eq!(src_box.box_size(), header.size);

        let dst_box = OpusBox::read_box(&mut reader, header.size).unwrap();
        assert_eq!(src_box, dst_box);
    }
}
