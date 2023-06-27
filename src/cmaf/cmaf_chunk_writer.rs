use byteorder::{BigEndian, WriteBytesExt};
use std::io::{Seek, SeekFrom, Write};
use std::time::Duration;

use crate::mfhd::MfhdBox;
use crate::mp4box::traf::TrafBox;

use crate::tfhd::TfhdBox;
use crate::trun::TrunBox;
use crate::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CmafChunkConfig {
    pub timescale: u32,
    pub default_sample_duration: u32,
    pub default_sample_size: u32,
    pub default_sample_flags: u32,
}

impl From<MediaConfig> for CmafChunkConfig {
    fn from(media_conf: MediaConfig) -> Self {
        match media_conf {
            MediaConfig::AvcConfig(avc_conf) => Self::from(avc_conf),
            MediaConfig::HevcConfig(hevc_conf) => Self::from(hevc_conf),
            MediaConfig::AacConfig(aac_conf) => Self::from(aac_conf),
            MediaConfig::TtxtConfig(ttxt_conf) => Self::from(ttxt_conf),
            MediaConfig::Vp9Config(vp9_config) => Self::from(vp9_config),
        }
    }
}

impl From<AvcConfig> for CmafChunkConfig {
    fn from(avc_conf: AvcConfig) -> Self {
        Self {
            timescale: 1000, // XXX
            default_sample_duration: 0,
            default_sample_size: 0,
            default_sample_flags: 0,
        }
    }
}

impl From<HevcConfig> for CmafChunkConfig {
    fn from(hevc_conf: HevcConfig) -> Self {
        Self {
            timescale: 1000, // XXX
            default_sample_duration: 0,
            default_sample_size: 0,
            default_sample_flags: 0,
        }
    }
}

impl From<AacConfig> for CmafChunkConfig {
    fn from(aac_conf: AacConfig) -> Self {
        Self {
            timescale: 1000, // XXX
            default_sample_duration: 0,
            default_sample_size: 0,
            default_sample_flags: 0,
        }
    }
}

impl From<TtxtConfig> for CmafChunkConfig {
    fn from(txtt_conf: TtxtConfig) -> Self {
        Self {
            timescale: 1000, // XXX
            default_sample_duration: 0,
            default_sample_size: 0,
            default_sample_flags: 0,
        }
    }
}

impl From<Vp9Config> for CmafChunkConfig {
    fn from(vp9_conf: Vp9Config) -> Self {
        Self {
            timescale: 1000, // XXX
            default_sample_duration: 0,
            default_sample_size: 0,
            default_sample_flags: 0,
        }
    }
}

// TODO creation_time, modification_time
#[derive(Debug, Default)]
pub struct CmafChunkWriter<W> {
    writer: W,
    traf: TrafBox,
    mfhd: MfhdBox,
    samples: Vec<Bytes>,
    timescale: u32,
}

impl<W: Write + Seek> CmafChunkWriter<W> {
    pub fn write_start(
        writer: W,
        sequence_number: u32,
        track_id: u32,
        config: &CmafChunkConfig,
    ) -> Result<Self> {
        let tfhd = TfhdBox {
            track_id,
            flags: TfhdBox::FLAG_DEFAULT_SAMPLE_FLAGS
                | TfhdBox::FLAG_DEFAULT_SAMPLE_DURATION
                | TfhdBox::FLAG_DEFAULT_SAMPLE_SIZE,
            default_sample_flags: Some(config.default_sample_flags),
            default_sample_duration: Some(config.default_sample_duration),
            default_sample_size: Some(config.default_sample_size),
            ..TfhdBox::default()
        };

        let traf = TrafBox {
            tfhd,
            tfdt: None,
            trun: None,
        };

        let mfhd = MfhdBox {
            flags: 0,
            version: 0,
            sequence_number,
        };

        Ok(CmafChunkWriter {
            writer,
            traf,
            mfhd,
            samples: vec![],
            timescale: config.timescale,
        })
    }

    pub fn duration(&self) -> Duration {
        if let Some(ref trun) = self.traf.trun {
            return Duration::from_micros(
                trun.duration() as u64 * 1_000_000 / self.timescale as u64,
            );
        }
        Duration::ZERO
    }

    fn sample_trun_flags(sample: &Mp4Sample) -> u32 {
        if sample.is_sync {
            TrunBox::FLAG_SAMPLE_DEPENDS_NO
        } else {
            TrunBox::FLAG_SAMPLE_DEPENDS_YES | TrunBox::FLAG_SAMPLE_FLAG_IS_NON_SYNC
        }
    }

    pub fn write_sample(&mut self, sample: &Mp4Sample) -> Result<u64> {
        self.samples.push(sample.bytes.clone());
        self.traf.tfdt.get_or_insert(tfdt::TfdtBox {
            version: 1,
            flags: 0, // ???
            base_media_decode_time: sample.start_time,
        });
        let sample_trun_flags = Self::sample_trun_flags(sample);
        let has_first_sample_flags = Some(sample_trun_flags) != self.traf.tfhd.default_sample_flags;
        let trun = self.traf.trun.get_or_insert(TrunBox {
            version: 1,
            data_offset: Some(0), // Temp value
            flags: TrunBox::FLAG_DATA_OFFSET
                | TrunBox::FLAG_SAMPLE_DURATION
                | TrunBox::FLAG_SAMPLE_SIZE,
            ..TrunBox::default()
        });

        if has_first_sample_flags && self.samples.len() == 1 {
            trun.flags |= TrunBox::FLAG_FIRST_SAMPLE_FLAGS;
            trun.first_sample_flags.get_or_insert(sample_trun_flags);
        }

        trun.sample_count = self.samples.len() as u32;
        trun.sample_durations.push(sample.duration);
        trun.sample_sizes.push(sample.bytes.len() as u32);
        trun.sample_cts.push(sample.rendering_offset as u32); // HNNN - fel typning här när trun är v1, kommer tolkas som en i32
        trun.sample_flags.push(sample_trun_flags);

        if trun.sample_cts.iter().any(|cts| *cts != 0) {
            trun.flags |= TrunBox::FLAG_SAMPLE_CTS;
        }

        let duration: u32 = trun.duration();
        Ok(duration as u64)
    }

    pub fn write_end(&mut self) -> Result<()> {
        let mut moof = MoofBox {
            mfhd: self.mfhd.clone(),
            trafs: vec![self.traf.clone()],
        };

        let moof_size = moof.get_size();

        if let Some(first) = moof.trafs.first_mut() {
            if let Some(ref mut trun) = first.trun {
                trun.data_offset = Some((moof_size + HEADER_SIZE) as i32);
            }
        }

        moof.write_box(&mut self.writer)?;

        let mdat_size = self.samples.iter().map(|s| s.len()).sum::<usize>();

        BoxHeader::new(BoxType::MdatBox, HEADER_SIZE + mdat_size as u64).write(&mut self.writer)?;

        for sample in self.samples.iter() {
            self.writer.write_all(sample)?;
        }

        Ok(())
    }

    pub fn into_writer(self) -> W {
        self.writer
    }
}

#[cfg(test)]
mod tests {

    use std::io::{Cursor, Read};

    use super::*;

    #[test]
    fn test_chunk() -> Result<()> {
        let config = CmafChunkConfig {
            timescale: 1000,
            default_sample_duration: 10,
            default_sample_size: 100,
            default_sample_flags: 0,
        };
        let data = Cursor::new(Vec::<u8>::new());

        let mut writer = CmafChunkWriter::write_start(data, 1, 1, &config)?;

        writer.write_sample(&Mp4Sample {
            start_time: 10,
            duration: 10,
            rendering_offset: 10,
            is_sync: true,
            bytes: Bytes::from_static(&[0, 0, 0, 0, 0, 0, 0]),
        })?;

        writer.write_end()?;

        let mut data: Vec<u8> = writer.into_writer().into_inner();

        let mut file = File::create("chunk.mp4").unwrap();
        file.write_all(&data).unwrap();

        let mut header = File::open("header.mp4").unwrap();
        let mut buffer = vec![0; header.metadata().unwrap().len() as usize];
        header.read_exact(&mut buffer).unwrap();
        buffer.append(&mut data);
        let size = data.len() as u64;
        let mp4 = Mp4Reader::read_header(Cursor::new(buffer), size)?;

        println!("{:?}", mp4);

        Ok(())
    }
}
