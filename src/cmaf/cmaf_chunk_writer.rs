use std::io::{Seek, Write};
use std::time::Duration;

use prft::PrftBox;

use crate::mfhd::{MfhdBox, MFHD_SIZE};
use crate::mp4box::traf::TrafBox;
use crate::psuedo_boxes::general_type_box::GeneralTypeBox;
use crate::styp::StypBox;
use crate::tfhd::TfhdBox;
use crate::trun::TrunBox;
use crate::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CmafChunkConfig {
    pub timescale: u32,
    pub default_sample_duration: u32,
    pub default_sample_size: u32,
    pub default_sample_flags: u32,
    pub producer_reference_time: Option<ProducerReferenceTime>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProducerReferenceTime {
    pub ntp_timestamp: u64,
    pub media_time: u64,
}

impl From<MediaConfig> for CmafChunkConfig {
    fn from(media_conf: MediaConfig) -> Self {
        match media_conf {
            MediaConfig::AvcConfig(avc_conf) => Self::from(avc_conf),
            MediaConfig::HevcConfig(hevc_conf) => Self::from(hevc_conf),
            MediaConfig::AacConfig(aac_conf) => Self::from(aac_conf),
            MediaConfig::TtxtConfig(ttxt_conf) => Self::from(ttxt_conf),
            MediaConfig::Vp9Config(vp9_config) => Self::from(vp9_config),
            MediaConfig::Av1Config(av1_config) => Self::from(av1_config),
            MediaConfig::OpusConfig(opus_config) => Self::from(opus_config),
        }
    }
}

impl From<AvcConfig> for CmafChunkConfig {
    fn from(_avc_conf: AvcConfig) -> Self {
        Self {
            timescale: 1000, // XXX
            default_sample_duration: 0,
            default_sample_size: 0,
            default_sample_flags: 0,
            producer_reference_time: None,
        }
    }
}

impl From<Av1Config> for CmafChunkConfig {
    fn from(_avc_conf: Av1Config) -> Self {
        Self {
            timescale: 1000, // XXX
            default_sample_duration: 0,
            default_sample_size: 0,
            default_sample_flags: 0,
            producer_reference_time: None,
        }
    }
}

impl From<HevcConfig> for CmafChunkConfig {
    fn from(_hevc_conf: HevcConfig) -> Self {
        Self {
            timescale: 1000, // XXX
            default_sample_duration: 0,
            default_sample_size: 0,
            default_sample_flags: 0,
            producer_reference_time: None,
        }
    }
}

impl From<AacConfig> for CmafChunkConfig {
    fn from(_aac_conf: AacConfig) -> Self {
        Self {
            timescale: 1000, // XXX
            default_sample_duration: 0,
            default_sample_size: 0,
            default_sample_flags: 0,
            producer_reference_time: None,
        }
    }
}

impl From<OpusConfig> for CmafChunkConfig {
    fn from(_opus_conf: OpusConfig) -> Self {
        Self {
            timescale: 1000, // XXX
            default_sample_duration: 0,
            default_sample_size: 0,
            default_sample_flags: 0,
            producer_reference_time: None,
        }
    }
}

impl From<TtxtConfig> for CmafChunkConfig {
    fn from(_txtt_conf: TtxtConfig) -> Self {
        Self {
            timescale: 1000, // XXX
            default_sample_duration: 0,
            default_sample_size: 0,
            default_sample_flags: 0,
            producer_reference_time: None,
        }
    }
}

impl From<Vp9Config> for CmafChunkConfig {
    fn from(_vp9_conf: Vp9Config) -> Self {
        Self {
            timescale: 1000, // XXX
            default_sample_duration: 0,
            default_sample_size: 0,
            default_sample_flags: 0,
            producer_reference_time: None,
        }
    }
}

// TODO creation_time, modification_time
#[derive(Debug)]
pub struct CmafChunkWriter<W> {
    writer: W,
    traf: TrafBox,
    mfhd: MfhdBox,
    prft: Option<PrftBox>,
    styp: Option<StypBox>,
    emsgs: Vec<EmsgBox>,
    samples: Vec<Bytes>,
    timescale: u32,
}

impl<W: Write + Seek> CmafChunkWriter<W> {
    pub fn write_start(writer: W, track_id: u32, config: &CmafChunkConfig) -> Result<Self> {
        let tfhd = TfhdBox {
            track_id,
            flags: TfhdBox::FLAG_DEFAULT_SAMPLE_FLAGS
                | TfhdBox::FLAG_DEFAULT_SAMPLE_DURATION
                | TfhdBox::FLAG_DEFAULT_SAMPLE_SIZE
                | TfhdBox::FLAG_DEFAULT_BASE_IS_MOOF, // Required for DRM in Safari
            default_sample_flags: Some(config.default_sample_flags),
            default_sample_duration: Some(config.default_sample_duration),
            default_sample_size: Some(config.default_sample_size),
            ..TfhdBox::default()
        };

        let traf = TrafBox {
            tfhd,
            tfdt: None,
            trun: None,
            senc: None,
            saiz: None,
            saio: None,
        };

        let mfhd = MfhdBox {
            flags: 0,
            version: 0,
            sequence_number: 0, // This is only a placeholder, the actual value will be set in write_end
        };

        let prft = config.producer_reference_time.as_ref().map(|prt| PrftBox {
            version: 1,
            flags: 0,
            reference_track_id: track_id,
            ntp_timestamp: prt.ntp_timestamp,
            media_time: prt.media_time,
        });

        Ok(CmafChunkWriter {
            writer,
            traf,
            mfhd,
            prft,
            styp: None,
            emsgs: vec![],
            samples: vec![],
            timescale: config.timescale,
        })
    }

    pub fn mark_new_segment(&mut self, cmaf_header_config: CmafHeaderConfig) {
        self.styp = Some(StypBox(GeneralTypeBox {
            major_brand: cmaf_header_config.major_brand,
            minor_version: cmaf_header_config.minor_version,
            compatible_brands: cmaf_header_config.compatible_brands,
        }));
    }

    pub fn producer_reference_time(&self) -> Option<&PrftBox> {
        self.prft.as_ref()
    }

    pub fn duration(&self) -> Duration {
        if let Some(ref trun) = self.traf.trun {
            return Duration::from_micros(
                trun.duration() as u64 * 1_000_000 / self.timescale as u64,
            );
        }
        Duration::ZERO
    }

    pub fn base_media_decode_time(&self) -> u64 {
        if let Some(ref tfdt) = self.traf.tfdt {
            return tfdt.base_media_decode_time;
        }

        0
    }

    pub fn starts_with_keyframe(&self) -> bool {
        let Some(ref trun) = self.traf.trun else {
            return false;
        };

        if let Some(first_sample_flags) = trun
            .first_sample_flags
            .or(trun.sample_flags.first().cloned())
        {
            return first_sample_flags & TrunBox::FLAG_SAMPLE_DEPENDS_NO > 0;
        }

        false
    }

    pub fn contains_keyframe(&self) -> bool {
        let Some(ref trun) = self.traf.trun else {
            return false;
        };

        trun.sample_flags
            .iter()
            .any(|flags| flags & TrunBox::FLAG_SAMPLE_DEPENDS_NO > 0)
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

        if let Some(encryption) = &sample.encryption {
            let use_subsample_encryption = !encryption.subsamples.is_empty();
            let senc = self.traf.senc.get_or_insert(senc::SencBox::new(
                use_subsample_encryption,
                encryption
                    .initialization_vector
                    .as_ref()
                    .map_or(0, |iv| iv.size()),
            ));
            senc.add_iv(encryption.clone());

            if use_subsample_encryption {
                self.traf.saio.get_or_insert(saio::SaioBox::new(0));

                let saiz = self.traf.saiz.get_or_insert(saiz::SaizBox::new(0));

                saiz.add_sample_info_size(2 + 6 * encryption.sub_samples().len() as u8);

                // It's important that the saio offset is updated after all other fields have been set
                let offset = HEADER_SIZE + MFHD_SIZE + self.traf.get_saio_offset();
                self.traf
                    .saio
                    .as_mut()
                    .expect("guaranteed insert above")
                    .set_single_offset(offset);
            }
        }

        Ok(duration as u64)
    }

    pub fn add_emsg(&mut self, emsg: EmsgBox) {
        self.emsgs.push(emsg);
    }

    pub fn write_end(&mut self, sequence_number: u32) -> Result<()> {
        if let Some(styp) = self.styp.as_ref() {
            styp.write_box(&mut self.writer)?;
        }

        self.mfhd.sequence_number = sequence_number;

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

        for emsg in self.emsgs.iter() {
            emsg.write_box(&mut self.writer)?;
        }

        if let Some(prft) = self.prft.as_ref() {
            prft.write_box(&mut self.writer)?;
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

    use std::io::Cursor;

    use super::*;

    #[test]
    fn test_chunk() -> Result<()> {
        let config = CmafHeaderConfig {
            major_brand: str::parse("iso6").unwrap(),
            minor_version: 512,
            compatible_brands: vec![
                str::parse("iso6").unwrap(),
                str::parse("cmfc").unwrap(),
                str::parse("mp41").unwrap(),
            ],
            timescale: 1000,
            pssh: Vec::new(),
        };
        let data = Cursor::new(Vec::<u8>::new());

        let mut writer = CmafHeaderWriter::write_start(data, &config, None)?;

        writer.add_track(&TrackConfig {
            track_type: TrackType::Video,
            timescale: 1000,
            language: "finne".to_string(),
            media_conf: MediaConfig::AvcConfig(AvcConfig {
                width: 1920,
                height: 1080,
                seq_param_set: [
                    103, 66, 192, 31, 149, 160, 20, 1, 110, 192, 90, 128, 128, 128, 160, 0, 0, 125,
                    0, 0, 29, 76, 28, 0, 0, 4, 196, 176, 0, 2, 98, 90, 221, 229, 193, 64,
                ]
                .to_vec(),
                pic_param_set: [104, 206, 60, 128].to_vec(),
                color: Some(ColorConfig {
                    color_primaries: 1,
                    transfer_characteristics: 1,
                    matrix_coefficients: 1,
                    full_range: false,
                }),
                aspect_ratio: Some((1, 1)),
            }),
            sinf: Vec::new(),
        })?;

        writer.write_end()?;

        let data = writer.into_writer().into_inner();

        let config = CmafChunkConfig {
            timescale: 1000,
            default_sample_duration: 10,
            default_sample_size: 100,
            default_sample_flags: 0,
            producer_reference_time: None,
        };
        let size = data.len();
        let mut data = Cursor::new(data);
        data.set_position(size as u64);

        let mut writer = CmafChunkWriter::write_start(data, 1, &config)?;

        writer.write_sample(&Mp4Sample {
            start_time: 10,
            duration: 10,
            rendering_offset: 10,
            is_sync: true,
            bytes: Bytes::from_static(&[0, 0, 0, 0, 0, 0, 0]),
            encryption: None,
        })?;

        writer.write_end(1)?;

        let data: Vec<u8> = writer.into_writer().into_inner();

        let size = data.len() as u64;

        Mp4Reader::read_header(Cursor::new(data), size)?;

        Ok(())
    }
}
