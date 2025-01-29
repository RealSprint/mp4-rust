use std::io::{Read, Seek};

use crate::{
    box_start, sinf::SinfBox, BoxHeader, BoxType, Error, Mp4Context, ReadBox, Result, HEADER_SIZE,
};

// Video Sample Entry
// reserved u8[6]             =  6 bytes (from Sample Entry)
// data_reference_index (u16) =  2 bytes (from Sample Entry)
// predefined (u16)           =  2 bytes
// reserved (u16)             =  2 bytes
// predefined (u32[3])        = 12 bytes
// width (u16) + height (u16) =  4 bytes
// horizresolution (u32)      =  4 bytes
// vertresolution (u32)       =  4 bytes
// reserved (u32)             =  4 bytes
// frame_count (u16)          =  2 bytes
// compressorname (u8[32])    = 32 bytes
// depth (u16)                =  2 bytes
// predefined (i16)           =  2 bytes
//                            = 78 bytes

const VIDEO_DATA_SIZE: i64 = 78;

// Audio Sample Entry v1
// reserved u8[6]              = 6 bytes (from Sample Entry)
// data_reference_index (u16)  = 2 bytes (from Sample Entry)
// entry_version (u16)         = 2 bytes
// reserved (u16)[3]           = 6 bytes
// channelcount (u16)          = 2 bytes
// samplesize (u16)            = 2 bytes
// pre_defined (u16)           = 2 bytes
// reserved (u16)              = 2 bytes
// samplerate (u32)            = 4 bytes
//                             = 28 bytes

// Audio Sample Entry
// reserved u8[6]              = 6 bytes (from Sample Entry)
// data_reference_index (u16)  = 2 bytes (from Sample Entry)
// reserved (u32)[2]           = 8 bytes
// channelcount (u16)          = 2 bytes
// samplesize (u16)            = 2 bytes
// pre_defined (u16)           = 2 bytes
// reserved (u16)              = 2 bytes
// samplerate (u32)            = 4 bytes
//                             = 28 bytes

// Both v0 and v1 have the same size
const AUDIO_DATA_SIZE: i64 = 28;

pub enum SampleEntryType {
    Video,
    Audio,
}

/// Try to determine the codec type of the video or audio track.
/// This function will read the current box and then search for the `frma` box (in the `sinf` box) to determine the codec type.
pub fn get_box_type<R: Read + Seek>(
    reader: &mut R,
    size: u64,
    kind: SampleEntryType,
    context: &mut Mp4Context,
) -> Result<BoxType> {
    let pos = reader.stream_position()?;
    let start = box_start(reader)?;
    let end = start + size;

    // Skip data related to the sample entry
    let data_size = match kind {
        SampleEntryType::Video => VIDEO_DATA_SIZE,
        SampleEntryType::Audio => AUDIO_DATA_SIZE,
    };
    reader.seek_relative(data_size)?;

    // There should only be boxes after this point.
    // We want to find frma box (in the sinf box) to determine the codec type.
    let sinf_header = loop {
        let header = BoxHeader::read(reader)?;
        if header.name == BoxType::SinfBox {
            break header;
        }

        reader.seek_relative((header.size - HEADER_SIZE) as i64)?;

        if reader.stream_position()? >= end {
            return Err(Error::InvalidData("Could not find sinf box"));
        }
    };

    let sinf = SinfBox::read_box(reader, sinf_header.size, context)?;

    let data_format: u32 = sinf.frma.data_format.into();

    reader.seek(std::io::SeekFrom::Start(pos))?;

    match BoxType::from(data_format) {
        BoxType::UnknownBox(_) => Err(Error::InvalidData("Unknown codec type")),
        b => Ok(b),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{mp4box::BoxHeader, BoxType};
    use std::io::Cursor;

    #[test]
    fn test_encoded_video() {
        let buf: Vec<u8> = vec![
            // encv
            0x00, 0x00, 0x00, 0xed, b'e', b'n', b'c', b'v', //
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, //
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
            0x04, 0xfe, 0x02, 0xd0, 0x00, 0x48, 0x00, 0x00, //
            0x00, 0x48, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
            0x00, 0x01, 0x0a, 0x41, 0x56, 0x43, 0x20, 0x43, //
            0x6f, 0x64, 0x69, 0x6e, 0x67, 0x00, 0x00, 0x00, //
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
            0x00, 0x00, 0x00, 0x18, 0xff, 0xff, //
            // avcC
            0x00, 0x00, 0x00, 0x34, b'a', b'v', b'c', b'C', //
            0x01, 0x4d, 0x40, 0x28, 0xff, 0xe1, 0x00, 0x1c, //
            0x67, 0x4d, 0x40, 0x28, 0xec, 0xa0, 0x28, 0x02, //
            0xdf, 0x5e, 0x02, 0xd4, 0x04, 0x04, 0x05, 0x00, //
            0x00, 0x03, 0x03, 0xe9, 0x00, 0x00, 0xea, 0x60, //
            0x8f, 0x18, 0x31, 0x96, 0x01, 0x00, 0x05, 0x68, //
            0xea, 0xe1, 0x32, 0xc8, //
            // colr (in avcC)
            0x00, 0x00, 0x00, 0x13, b'c', b'o', b'l', b'r', //
            0x6e, 0x63, 0x6c, 0x78, 0x00, 0x01, 0x00, 0x01, //
            0x00, 0x01, 0x00, //
            // sinf
            0x00, 0x00, 0x00, 0x50, b's', b'i', b'n', b'f', //
            // frma (in sinf)
            0x00, 0x00, 0x00, 0x0c, b'f', b'r', b'm', b'a', //
            0x61, 0x76, 0x63, 0x31, //
            // schm
            0x00, 0x00, 0x00, 0x14, b's', b'c', b'h', b'm', //
            0x00, 0x00, 0x00, 0x00, 0x63, 0x65, 0x6e, 0x63, //
            0x00, 0x01, 0x00, 0x00, //
            // schi
            0x00, 0x00, 0x00, 0x28, b's', b'c', b'h', b'i', //
            // tenc in (schi)
            0x00, 0x00, 0x00, 0x20, b't', b'e', b'n', b'c', //
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x10, //
            0x6d, 0x76, 0xf2, 0x5c, 0xb1, 0x7f, 0x5e, 0x16, //
            0xb8, 0xea, 0xef, 0x6b, 0xbf, 0x58, 0x2d, 0x8e, //
        ];

        let mut reader = Cursor::new(&buf);
        let header = BoxHeader::read(&mut reader).unwrap();
        assert_eq!(header.name, BoxType::EncvBox);

        let original_position = reader.stream_position().unwrap();
        let box_type = get_box_type(
            &mut reader,
            header.size,
            SampleEntryType::Video,
            &mut Mp4Context::default(),
        )
        .unwrap();
        assert_eq!(box_type, BoxType::Avc1Box);
        assert_eq!(reader.stream_position().unwrap(), original_position);
    }

    #[test]
    fn test_encoded_audio() {
        let buf: Vec<u8> = vec![
            // enca
            0x00, 0x00, 0x00, 0x9E, 0x65, 0x6E, 0x63, 0x61, //
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, //
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
            0x00, 0x02, 0x00, 0x10, 0x00, 0x00, 0x00, 0x00, //
            0xBB, 0x80, 0x00, 0x00, //
            // esds
            0x00, 0x00, 0x00, 0x2a, 0x65, 0x73, 0x64, 0x73, //
            0x00, 0x00, 0x00, 0x00, 0x03, 0x1C, 0x00, 0x00, //
            0x00, 0x04, 0x14, 0x40, 0x15, 0x00, 0x00, 0x00, //
            0x00, 0x01, 0x39, 0xA4, 0x00, 0x01, 0x39, 0xA4, //
            0x05, 0x05, 0x11, 0x90, 0x56, 0xE5, 0x00, 0x06, //
            0x01, 0x02, //
            // sinf
            0x00, 0x00, 0x00, 0x50, b's', b'i', b'n', b'f', //
            // frma (in sinf)
            0x00, 0x00, 0x00, 0x0C, b'f', b'r', b'm', b'a', //
            b'm', b'p', b'4', b'a', //
            // schm (in sinf)
            0x00, 0x00, 0x00, 0x14, b's', b'c', b'h', b'm', //
            0x00, 0x00, 0x00, 0x00, 0x63, 0x65, 0x6E, 0x63, //
            0x00, 0x01, 0x00, 0x00, //
            // schi (in sinf)
            0x00, 0x00, 0x00, 0x28, b's', b'c', b'h', b'i', //
            // tenc in (schi)
            0x00, 0x00, 0x00, 0x20, b't', b'e', b'n', b'c', //
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x10, //
            0x6D, 0x76, 0xF2, 0x5C, 0xB1, 0x7F, 0x5E, 0x16, //
            0xB8, 0xEA, 0xEF, 0x6B, 0xBF, 0x58, 0x2D, 0x8E, //
        ];

        let mut reader = Cursor::new(&buf);
        let header = BoxHeader::read(&mut reader).unwrap();
        assert_eq!(header.name, BoxType::EncaBox);

        let original_position = reader.stream_position().unwrap();
        let box_type = get_box_type(
            &mut reader,
            header.size,
            SampleEntryType::Audio,
            &mut Mp4Context::default(),
        )
        .unwrap();
        assert_eq!(box_type, BoxType::Mp4aBox);
        assert_eq!(reader.stream_position().unwrap(), original_position);
    }
}
