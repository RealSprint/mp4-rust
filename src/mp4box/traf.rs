use senc::SencBox;
use serde::Serialize;
use std::io::{Read, Seek, Write};
use tracing::debug;

use crate::mp4box::*;
use crate::mp4box::{tfdt::TfdtBox, tfhd::TfhdBox, trun::TrunBox};

use super::saio::SaioBox;
use super::saiz::SaizBox;

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct TrafBox {
    pub tfhd: TfhdBox,
    pub tfdt: Option<TfdtBox>,
    pub trun: Option<TrunBox>,

    pub senc: Option<SencBox>,
    pub saiz: Option<SaizBox>,
    pub saio: Option<SaioBox>,
}

impl TrafBox {
    pub fn get_type(&self) -> BoxType {
        BoxType::TrafBox
    }

    pub fn get_size(&self) -> u64 {
        let mut size = HEADER_SIZE;
        size += self.tfhd.box_size();
        if let Some(ref tfdt) = self.tfdt {
            size += tfdt.box_size();
        }
        if let Some(ref trun) = self.trun {
            size += trun.box_size();
        }
        if let Some(ref saiz) = self.saiz {
            size += saiz.box_size();
        }
        if let Some(ref saio) = self.saio {
            size += saio.box_size();
        }
        if let Some(ref senc) = self.senc {
            size += senc.box_size();
        }
        size
    }

    /// Gets offset to be used in saio box.
    /// It's the offset from the begging of the moof box to 16 bytes
    /// into the senc box (where the sample data begins).
    pub fn get_saio_offset(&self) -> u64 {
        let Some(ref senc) = self.senc else {
            return 0;
        };

        let mut end = self.get_size();
        end -= senc.box_size();
        end += 16; // 16 bytes into the senc box
        end
    }
}

impl Mp4Box for TrafBox {
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
        let s = String::new();
        Ok(s)
    }
}

impl<R: Read + Seek> ReadBox<&mut R> for TrafBox {
    fn read_box(reader: &mut R, size: u64, context: &mut Mp4Context) -> Result<Self> {
        let start = box_start(reader)?;

        let mut tfhd = None;
        let mut tfdt = None;
        let mut trun = None;
        let mut senc = None;
        let mut saiz = None;
        let mut saio = None;

        let mut current = reader.stream_position()?;
        let end = start + size;

        // The order here is important, as we need to know the track ID before reading the senc box.
        let mut boxes: HashMap<BoxType, u64> = HashMap::new();
        while current < end {
            // Get box header.
            let header = BoxHeader::read(reader)?;
            let BoxHeader { name, size: s } = header;
            if s > size {
                return Err(Error::InvalidData(
                    "traf box contains a box with a larger size than it",
                ));
            }
            boxes.insert(name, current);
            skip_box(reader, s)?;
            current = reader.stream_position()?;
        }

        if let Some(tfhd_start) = boxes.remove(&BoxType::TfhdBox) {
            reader.seek(SeekFrom::Start(tfhd_start))?;
            let header = BoxHeader::read(reader)?;
            let BoxHeader { name: _, size: s } = header;
            tfhd = Some(TfhdBox::read_box(reader, s, context)?);
        }

        if tfhd.is_none() {
            return Err(Error::BoxNotFound(BoxType::TfhdBox));
        }

        if let Some(tfdt_start) = boxes.remove(&BoxType::TfdtBox) {
            reader.seek(SeekFrom::Start(tfdt_start))?;
            let header = BoxHeader::read(reader)?;
            let BoxHeader { name: _, size: s } = header;
            tfdt = Some(TfdtBox::read_box(reader, s, context)?);
        }

        if let Some(trun_start) = boxes.remove(&BoxType::TrunBox) {
            reader.seek(SeekFrom::Start(trun_start))?;
            let header = BoxHeader::read(reader)?;
            let BoxHeader { name: _, size: s } = header;
            trun = Some(TrunBox::read_box(reader, s, context)?);
        }

        if let Some(senc_start) = boxes.remove(&BoxType::SencBox) {
            let track_id = tfhd.as_ref().expect("checked above").track_id;

            reader.seek(SeekFrom::Start(senc_start))?;
            let header = BoxHeader::read(reader)?;
            let BoxHeader { name: _, size: s } = header;
            senc = Some(SencBox::read_box(reader, s, context, track_id)?);
        }

        if let Some(saiz_start) = boxes.remove(&BoxType::SaizBox) {
            reader.seek(SeekFrom::Start(saiz_start))?;
            let header = BoxHeader::read(reader)?;
            let BoxHeader { name: _, size: s } = header;
            saiz = Some(SaizBox::read_box(reader, s, context)?);
        }

        if let Some(saio_start) = boxes.remove(&BoxType::SaioBox) {
            reader.seek(SeekFrom::Start(saio_start))?;
            let header = BoxHeader::read(reader)?;
            let BoxHeader { name: _, size: s } = header;
            saio = Some(SaioBox::read_box(reader, s, context)?);
        }

        for (name, _) in boxes {
            debug!("Skipping box: {:?}", name);
        }

        skip_bytes_to(reader, start + size)?;

        Ok(TrafBox {
            tfhd: tfhd.unwrap(),
            tfdt,
            trun,
            senc,
            saiz,
            saio,
        })
    }
}

impl<W: Write> WriteBox<&mut W> for TrafBox {
    fn write_box(&self, writer: &mut W) -> Result<u64> {
        let size = self.box_size();
        BoxHeader::new(self.box_type(), size).write(writer)?;

        self.tfhd.write_box(writer)?;

        for tfdt in self.tfdt.iter() {
            tfdt.write_box(writer)?;
        }

        for trun in self.trun.iter() {
            trun.write_box(writer)?;
        }

        if let Some(ref saiz) = self.saiz {
            saiz.write_box(writer)?;
        }

        if let Some(ref saio) = self.saio {
            saio.write_box(writer)?;
        }

        if let Some(ref senc) = self.senc {
            senc.write_box(writer)?;
        }

        Ok(size)
    }
}
