use std::io::{Cursor, Read, Seek, Write};

use base64::{prelude::BASE64_STANDARD, Engine};
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use serde::Serialize;

use super::{
    box_start, skip_bytes_to, write_box_header_ext, BoxHeader, BoxType, Error, Mp4Box, Mp4Context,
    ReadBox, Result, WriteBox, HEADER_EXT_SIZE, HEADER_SIZE,
};

// ISO 23001-7:2023 - 8.1 Protection System Specific Header Box
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct PsshBox {
    version: u8,
    flags: u32,

    system_id: [u8; 16],

    kid_count: Option<u32>,
    kid: Vec<[u8; 16]>,

    data_size: u32,
    data: Vec<u8>,
}

impl PsshBox {
    pub fn from_base64(base64: &str) -> Result<Self> {
        let data = BASE64_STANDARD.decode(base64.as_bytes())?;

        let mut reader = Cursor::new(&data);
        let header = BoxHeader::read(&mut reader)?;

        PsshBox::read_box(&mut reader, header.size, &mut Mp4Context::default())
    }

    pub fn new_clearkey() -> Self {
        let system_id = [
            0x10, 0x77, 0xef, 0xec, 0xc0, 0xb2, 0x4d, 0x02, //
            0xac, 0xe3, 0x3c, 0x1e, 0x52, 0xe2, 0xfb, 0x4b, //
        ];
        Self::new(system_id, vec![])
    }

    pub fn new(system_id: [u8; 16], data: Vec<u8>) -> Self {
        PsshBox {
            version: 0,
            flags: 0,

            system_id,

            kid_count: None,
            kid: Vec::new(),

            data_size: data.len() as u32,
            data,
        }
    }

    pub fn with_kid(system_id: [u8; 16], kid: Vec<[u8; 16]>, data: Vec<u8>) -> Self {
        PsshBox {
            version: 1,
            flags: 0,

            system_id,

            kid_count: Some(kid.len() as u32),
            kid,

            data_size: data.len() as u32,
            data,
        }
    }

    pub fn get_kid(&self) -> &Vec<[u8; 16]> {
        &self.kid
    }

    pub fn get_system_id(&self) -> &[u8; 16] {
        &self.system_id
    }

    pub fn get_type(&self) -> BoxType {
        BoxType::PsshBox
    }

    pub fn get_size(&self) -> u64 {
        let kid_size = if self.version > 0 {
            let kid_count = self.kid_count.expect("always set for version > 0") as u64;
            4 + 16 * kid_count
        } else {
            0
        };

        let data_size = 4 + self.data_size as u64;

        HEADER_SIZE + HEADER_EXT_SIZE + 16 + kid_size + data_size
    }
}

impl Mp4Box for PsshBox {
    fn box_type(&self) -> BoxType {
        self.get_type()
    }

    fn box_size(&self) -> u64 {
        self.get_size()
    }

    fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string(&self).unwrap_or_default())
    }

    fn summary(&self) -> Result<String> {
        let s = format!(
            "system_id={:?}, kid_count={:?}, kid={:?}, data_size={}, data={:?}",
            self.system_id, self.kid_count, self.kid, self.data_size, self.data
        );
        Ok(s)
    }
}

impl<R: Read + Seek> ReadBox<&mut R> for PsshBox {
    fn read_box(reader: &mut R, size: u64, _context: &mut Mp4Context) -> Result<Self> {
        let start = box_start(reader)?;

        let (version, flags) = super::read_box_header_ext(reader)?;

        let mut system_id = [0; 16];
        reader.read_exact(&mut system_id)?;

        let (kid_count, kid) = if version > 0 {
            let kid_count = reader.read_u32::<BigEndian>()?;
            let mut kid = vec![[0; 16]; kid_count as usize];
            for i in 0..kid_count {
                reader.read_exact(&mut kid[i as usize])?;
            }
            (Some(kid_count), kid)
        } else {
            (None, Vec::new())
        };

        let data_size = reader.read_u32::<BigEndian>()?;
        let mut data = vec![0; data_size as usize];
        reader.read_exact(&mut data)?;

        skip_bytes_to(reader, start + size)?;

        Ok(PsshBox {
            version,
            flags,
            system_id,
            kid_count,
            kid,
            data_size,
            data,
        })
    }
}

impl<W: Write> WriteBox<&mut W> for PsshBox {
    fn write_box(&self, writer: &mut W) -> Result<u64> {
        let size = self.box_size();

        BoxHeader::new(self.box_type(), size).write(writer)?;

        write_box_header_ext(writer, self.version, self.flags)?;

        writer.write_all(&self.system_id)?;

        if self.version > 0 {
            let kid_count = match self.kid_count {
                Some(kid_count) => kid_count,
                None => return Err(Error::InvalidData("kid_count is required for version > 0")),
            };

            writer.write_u32::<BigEndian>(kid_count)?;

            for i in 0..kid_count {
                writer.write_all(&self.kid[i as usize])?;
            }
        }

        writer.write_u32::<BigEndian>(self.data_size)?;
        writer.write_all(&self.data)?;

        Ok(size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mp4box::BoxHeader;
    use std::io::Cursor;

    #[test]
    fn test_pssh() {
        let system_id = [
            0x10, 0x77, 0xef, 0xec, 0xc0, 0xb2, 0x4d, 0x02, //
            0xac, 0xe3, 0x3c, 0x1e, 0x52, 0xe2, 0xfb, 0x4b,
        ];

        let data = vec![
            0x6d, 0x76, 0xf2, 0x5c, 0xb1, 0x7f, 0x5e, 0x16, //
        ];

        let src_box = PsshBox::new(system_id, data);

        let mut buf = Vec::new();
        src_box.write_box(&mut buf).unwrap();
        assert_eq!(buf.len(), src_box.box_size() as usize);

        let expected: Vec<u8> = vec![
            0x00, 0x00, 0x00, 0x28, b'p', b's', b's', b'h', // header
            0x00, 0x00, 0x00, 0x00, // header ext
            0x10, 0x77, 0xef, 0xec, 0xc0, 0xb2, 0x4d, 0x02, // system id
            0xac, 0xe3, 0x3c, 0x1e, 0x52, 0xe2, 0xfb, 0x4b, // system id
            0x00, 0x00, 0x00, 0x08, // data size
            0x6d, 0x76, 0xf2, 0x5c, 0xb1, 0x7f, 0x5e, 0x16, // data
        ];
        assert_eq!(buf, expected);

        let mut reader = Cursor::new(&buf);
        let header = BoxHeader::read(&mut reader).unwrap();
        assert_eq!(header.name, BoxType::PsshBox);
        assert_eq!(src_box.box_size(), header.size);

        let dst_box =
            PsshBox::read_box(&mut reader, header.size, &mut Mp4Context::default()).unwrap();
        assert_eq!(src_box, dst_box);
    }

    #[test]
    fn test_pssh_with_kid() {
        let kid = vec![[
            0x6d, 0x76, 0xf2, 0x5c, 0xb1, 0x7f, 0x5e, 0x16, //
            0xb8, 0xea, 0xef, 0x6b, 0xbf, 0x58, 0x2d, 0x8e,
        ]];

        let system_id = [
            0x10, 0x77, 0xef, 0xec, 0xc0, 0xb2, 0x4d, 0x02, //
            0xac, 0xe3, 0x3c, 0x1e, 0x52, 0xe2, 0xfb, 0x4b,
        ];

        let src_box = PsshBox::with_kid(system_id, kid, Vec::new());

        let mut buf = Vec::new();
        src_box.write_box(&mut buf).unwrap();
        assert_eq!(buf.len(), src_box.box_size() as usize);

        let expected: Vec<u8> = vec![
            0x00, 0x00, 0x00, 0x34, b'p', b's', b's', b'h', //
            0x01, 0x00, 0x00, 0x00, 0x10, 0x77, 0xef, 0xec, //
            0xc0, 0xb2, 0x4d, 0x02, 0xac, 0xe3, 0x3c, 0x1e, //
            0x52, 0xe2, 0xfb, 0x4b, 0x00, 0x00, 0x00, 0x01, //
            0x6d, 0x76, 0xf2, 0x5c, 0xb1, 0x7f, 0x5e, 0x16, //
            0xb8, 0xea, 0xef, 0x6b, 0xbf, 0x58, 0x2d, 0x8e, //
            0x00, 0x00, 0x00, 0x00,
        ];
        assert_eq!(buf, expected);

        let mut reader = Cursor::new(&buf);
        let header = BoxHeader::read(&mut reader).unwrap();
        assert_eq!(header.name, BoxType::PsshBox);
        assert_eq!(src_box.box_size(), header.size);

        let dst_box =
            PsshBox::read_box(&mut reader, header.size, &mut Mp4Context::default()).unwrap();
        assert_eq!(src_box, dst_box);
    }

    #[test]
    fn widevine() {
        let base64 =
            "AAAAP3Bzc2gAAAAA7e+LqXnWSs6jyCfc1R0h7QAAAB8SEKIFDqR9T0FWsI36mib6TasaBWV6ZHJtSPPGiZsG";
        let data = BASE64_STANDARD.decode(base64.as_bytes()).unwrap();

        let mut reader = Cursor::new(&data);
        let header = BoxHeader::read(&mut reader).unwrap();
        assert_eq!(header.name, BoxType::PsshBox);

        let dst_box =
            PsshBox::read_box(&mut reader, header.size, &mut Mp4Context::default()).unwrap();

        let k = 1;
        assert_eq!(k, dst_box.data_size)
    }

    #[test]
    fn playready() {
        let p = "000002be70737368000000009a04f07998404286ab92e65be0885f950000029e9e0200000100010094023c00570052004d00480045004100440045005200200078006d006c006e0073003d00220068007400740070003a002f002f0073006300680065006d00610073002e006d006900630072006f0073006f00660074002e0063006f006d002f00440052004d002f0032003000300037002f00300033002f0050006c00610079005200650061006400790048006500610064006500720022002000760065007200730069006f006e003d00220034002e0033002e0030002e00300022003e003c0044004100540041003e003c00500052004f00540045004300540049004e0046004f003e003c004b004900440053003e003c004b0049004400200041004c004700490044003d00220041004500530043004200430022002000560041004c00550045003d00220070004100340046006f006b003900390056006b00470077006a006600710061004a00760070004e00710077003d003d0022003e003c002f004b00490044003e003c002f004b004900440053003e003c002f00500052004f00540045004300540049004e0046004f003e003c004c0041005f00550052004c003e00680074007400700073003a002f002f0070006c0061007900720065006100640079002e0065007a00640072006d002e0063006f006d002f00630065006e00630079002f0070007200650061007500740068002e0061007300700078003f00700058003d004600360031003400440031003c002f004c0041005f00550052004c003e003c00440053005f00490044003e0056006c005200370049006400730049004a004500750052006400300036004c0061007100730032006a0077003d003d003c002f00440053005f00490044003e003c002f0044004100540041003e003c002f00570052004d004800450041004400450052003e00";
        let data = hex::decode(p).unwrap();
        let mut reader = Cursor::new(&data);
        let header = BoxHeader::read(&mut reader).unwrap();
        assert_eq!(header.name, BoxType::PsshBox);

        let dst_box =
            PsshBox::read_box(&mut reader, header.size, &mut Mp4Context::default()).unwrap();

        let k = 1;
        assert_eq!(k, dst_box.data_size)
    }
}
