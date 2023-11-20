use crate::EmsgBox;

pub enum EmsgKind {
    Id3(Vec<u8>),
}

pub struct EmsgData {
    pub kind: EmsgKind,
    pub timescale: u32,
    pub presentation_time: u64,
    pub event_duration: u32,
    pub id: u32,
}

impl EmsgData {
    pub fn build_box(&self) -> EmsgBox {
        EmsgBox {
            version: 1,
            flags: 0,
            timescale: self.timescale,
            presentation_time: Some(self.presentation_time),
            presentation_time_delta: None,
            event_duration: self.event_duration,
            id: self.id,
            scheme_id_uri: self.scheme_id_uri(),
            message_data: self.message_data(),
            value: "".to_string(),
        }
    }

    pub fn scheme_id_uri(&self) -> String {
        match &self.kind {
            EmsgKind::Id3(_) => "https://aomedia.org/emsg/ID3".to_string(),
        }
    }

    pub fn message_data(&self) -> Vec<u8> {
        match &self.kind {
            EmsgKind::Id3(data) => data.clone(),
        }
    }
}
