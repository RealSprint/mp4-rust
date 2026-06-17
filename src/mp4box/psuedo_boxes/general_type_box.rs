use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use serde::Serialize;
use std::io::{Read, Seek, Write};

use crate::mp4box::*;

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct GeneralTypeBox {
    pub major_brand: FourCC,
    pub minor_version: u32,
    pub compatible_brands: Vec<FourCC>,
}

pub enum GeneralTypeBoxType {
    FtypBox,
    StypBox,
}

impl GeneralTypeBox {
    pub fn get_size(&self) -> u64 {
        HEADER_SIZE + 8 + (4 * self.compatible_brands.len() as u64)
    }

    pub fn summary(&self) -> Result<String> {
        let mut compatible_brands = Vec::new();
        for brand in self.compatible_brands.iter() {
            compatible_brands.push(brand.to_string());
        }
        let s = format!(
            "major_brand={} minor_version={} compatible_brands={}",
            self.major_brand,
            self.minor_version,
            compatible_brands.join("-")
        );
        Ok(s)
    }

    pub fn write_box<W: Write>(&self, writer: &mut W, kind: GeneralTypeBoxType) -> Result<u64> {
        let box_type = match kind {
            GeneralTypeBoxType::FtypBox => BoxType::FtypBox,
            GeneralTypeBoxType::StypBox => BoxType::StypBox,
        };

        let size = self.get_size();
        BoxHeader::new(box_type, size).write(writer)?;

        writer.write_u32::<BigEndian>((&self.major_brand).into())?;
        writer.write_u32::<BigEndian>(self.minor_version)?;
        for b in self.compatible_brands.iter() {
            writer.write_u32::<BigEndian>(b.into())?;
        }
        Ok(size)
    }

    pub fn read_box<R: Read + Seek>(
        reader: &mut R,
        size: u64,
        _context: &mut Mp4Context,
    ) -> Result<Self> {
        let start = box_start(reader)?;

        // `u64::is_multiple_of` is stable only since Rust 1.87; the modulo form
        // keeps the crate's MSRV at 1.80 (bounded by `Seek::seek_relative`).
        #[allow(clippy::manual_is_multiple_of)]
        if size < 16 || size % 4 != 0 {
            return Err(Error::InvalidData(
                "ftyp/styp size too small or not aligned",
            ));
        }
        let brand_count = (size - 16) / 4; // header + major + minor
        let major = reader.read_u32::<BigEndian>()?;
        let minor = reader.read_u32::<BigEndian>()?;

        let mut brands = Vec::new();
        for _ in 0..brand_count {
            let b = reader.read_u32::<BigEndian>()?;
            brands.push(From::from(b));
        }

        skip_bytes_to(reader, start + size)?;

        Ok(GeneralTypeBox {
            major_brand: From::from(major),
            minor_version: minor,
            compatible_brands: brands,
        })
    }
}
