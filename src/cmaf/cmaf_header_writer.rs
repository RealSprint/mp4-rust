use std::io::{Seek, Write};

use crate::mp4box::*;
use crate::mvex::MvexBox;
use crate::track::Mp4TrackWriter;
use crate::trex::TrexBox;
use crate::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CmafHeaderConfig {
    pub major_brand: FourCC,
    pub minor_version: u32,
    pub compatible_brands: Vec<FourCC>,
    pub timescale: u32,
}

#[derive(Debug)]
pub struct CmafHeaderWriter<W> {
    writer: W,
    tracks: Vec<Mp4TrackWriter>,
    timescale: u32,
    duration: u64,
}

impl<W> CmafHeaderWriter<W> {
    /// Consume self, returning the inner writer.
    ///
    /// This can be useful to recover the inner writer after completion in case
    /// it's owned by the [CmafWriter] instance.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use mp4::{CmafWriter, CmafConfig};
    /// use std::io::Cursor;
    ///
    /// # fn main() -> mp4::Result<()> {
    /// let config = CmafConfig {
    ///     major_brand: str::parse("iso6").unwrap(),
    ///     minor_version: 512,
    ///     compatible_brands: vec![
    ///         str::parse("isom").unwrap(),
    ///         str::parse("iso2").unwrap(),
    ///         str::parse("avc1").unwrap(),
    ///         str::parse("mp41").unwrap(),
    ///     ],
    ///     timescale: 1000,
    /// };
    ///
    /// let data = Cursor::new(Vec::<u8>::new());
    /// let mut writer = mp4::CmafWriter::write_start(data, &config)?;
    /// writer.write_end()?;
    ///
    /// let data: Vec<u8> = writer.into_writer().into_inner();
    /// # Ok(()) }
    /// ```
    pub fn into_writer(self) -> W {
        self.writer
    }
}

impl<W: Write + Seek> CmafHeaderWriter<W> {
    pub fn write_start(mut writer: W, config: &CmafHeaderConfig) -> Result<Self> {
        let ftyp = FtypBox {
            major_brand: config.major_brand,
            minor_version: config.minor_version,
            compatible_brands: config.compatible_brands.clone(),
        };
        ftyp.write_box(&mut writer)?;

        let tracks = Vec::new();
        let timescale = config.timescale;
        let duration = 0;
        Ok(Self {
            writer,
            tracks,
            timescale,
            duration,
        })
    }

    pub fn add_track(&mut self, config: &TrackConfig) -> Result<()> {
        let track_id = self.tracks.len() as u32 + 1;
        let track = Mp4TrackWriter::new(track_id, config)?;
        self.tracks.push(track);
        Ok(())
    }

    pub fn write_end(&mut self) -> Result<()> {
        let mut moov = MoovBox {
            mvex: Some(MvexBox {
                mehd: None,
                trex: TrexBox {
                    version: 0,
                    flags: 0,
                    track_id: 1,
                    default_sample_description_index: 1,
                    default_sample_duration: 0,
                    default_sample_size: 0,
                    default_sample_flags: 0,
                },
            }),
            ..MoovBox::default()
        };

        for track in self.tracks.iter_mut() {
            moov.traks.push(track.write_end(&mut self.writer)?);
        }

        moov.mvhd.next_track_id = 2;
        moov.mvhd.timescale = self.timescale;
        moov.mvhd.duration = self.duration;
        if moov.mvhd.duration > (u32::MAX as u64) {
            moov.mvhd.version = 1
        }
        moov.write_box(&mut self.writer)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    use std::io::Cursor;

    use super::*;

    #[test]
    fn test_header() -> Result<()> {
        let config = CmafHeaderConfig {
            major_brand: str::parse("iso6").unwrap(),
            minor_version: 512,
            compatible_brands: vec![
                str::parse("iso6").unwrap(),
                str::parse("cmfc").unwrap(),
                str::parse("mp41").unwrap(),
            ],
            timescale: 1000,
        };
        let data = Cursor::new(Vec::<u8>::new());

        let mut writer = CmafHeaderWriter::write_start(data, &config)?;

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
            }),
        })?;

        writer.write_end()?;

        let data: Vec<u8> = writer.into_writer().into_inner();
        let size = data.len() as u64;

        let mut file = File::create("header.mp4").unwrap();
        file.write_all(&data).unwrap();

        let reader = BufReader::new(Cursor::new(data));
        let mp4 = Mp4Reader::read_header(reader, size)?;

        Ok(())
    }
}
