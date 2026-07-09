use crate::protocol::constants::*;
use crate::protocol::types::PacketType;

#[derive(Debug, Clone)]
pub struct PacketHeader {
    pub magic: [u8; 2],
    pub version: u8,
    pub packet_type: PacketType,
    pub sequence: u32,
    pub timestamp: u32,
    pub payload_len: u16,
}

#[derive(Debug, Clone)]
pub struct Packet {
    pub header: PacketHeader,
    pub payload: Vec<u8>,
}

impl Packet {
    pub fn new(packet_type: PacketType, sequence: u32, timestamp: u32, payload: Vec<u8>) -> Self {
        let payload_len = payload.len() as u16;
        Self {
            header: PacketHeader {
                magic: PROTOCOL_MAGIC,
                version: PROTOCOL_VERSION,
                packet_type,
                sequence,
                timestamp,
                payload_len,
            },
            payload,
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(HEADER_SIZE + self.payload.len());
        buf.extend_from_slice(&self.header.magic);
        buf.push(self.header.version);
        buf.push(self.header.packet_type as u8);
        buf.extend_from_slice(&self.header.sequence.to_be_bytes());
        buf.extend_from_slice(&self.header.timestamp.to_be_bytes());
        buf.extend_from_slice(&self.header.payload_len.to_be_bytes());
        buf.extend_from_slice(&self.payload);
        buf
    }

    pub fn decode(data: &[u8]) -> Result<Self, PacketError> {
        if data.len() < HEADER_SIZE {
            return Err(PacketError::TooShort(data.len()));
        }
        if data[0] != PROTOCOL_MAGIC[0] || data[1] != PROTOCOL_MAGIC[1] {
            return Err(PacketError::BadMagic);
        }
        let version = data[2];
        let packet_type =
            PacketType::from_u8(data[3]).ok_or(PacketError::UnknownType(data[3]))?;

        let mut seq_bytes = [0u8; 4];
        seq_bytes.copy_from_slice(&data[4..8]);
        let sequence = u32::from_be_bytes(seq_bytes);

        let mut ts_bytes = [0u8; 4];
        ts_bytes.copy_from_slice(&data[8..12]);
        let timestamp = u32::from_be_bytes(ts_bytes);

        let mut len_bytes = [0u8; 2];
        len_bytes.copy_from_slice(&data[12..14]);
        let payload_len = u16::from_be_bytes(len_bytes) as usize;

        let payload_start = 14;
        let payload_end = payload_start + payload_len;
        if payload_end > data.len() {
            return Err(PacketError::Truncated {
                expected: payload_len,
                actual: data.len() - payload_start,
            });
        }

        Ok(Self {
            header: PacketHeader {
                magic: [data[0], data[1]],
                version,
                packet_type,
                sequence,
                timestamp,
                payload_len: payload_len as u16,
            },
            payload: data[payload_start..payload_end].to_vec(),
        })
    }

    pub fn total_size(&self) -> usize {
        HEADER_SIZE + self.payload.len()
    }
}

#[derive(Debug, Clone)]
pub enum PacketError {
    TooShort(usize),
    BadMagic,
    UnknownType(u8),
    Truncated { expected: usize, actual: usize },
    InvalidPayload(&'static str),
}

impl std::fmt::Display for PacketError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooShort(n) => write!(f, "packet too short: {n} bytes"),
            Self::BadMagic => write!(f, "invalid magic bytes"),
            Self::UnknownType(t) => write!(f, "unknown packet type: 0x{t:02x}"),
            Self::Truncated { expected, actual } => {
                write!(f, "truncated payload: expected {expected}, got {actual}")
            }
            Self::InvalidPayload(msg) => write!(f, "invalid payload: {msg}"),
        }
    }
}

impl std::error::Error for PacketError {}

impl Packet {
    pub fn encode_audio_payload(
        channels: u16,
        sample_rate: u32,
        codec_id: u8,
        frame_data: &[u8],
    ) -> Vec<u8> {
        let mut payload = Vec::with_capacity(7 + frame_data.len());
        payload.extend_from_slice(&channels.to_be_bytes());
        payload.extend_from_slice(&sample_rate.to_be_bytes());
        payload.push(codec_id);
        payload.extend_from_slice(frame_data);
        payload
    }

    pub fn decode_audio_payload(payload: &[u8]) -> Result<(u16, u32, u8, &[u8]), PacketError> {
        if payload.len() < 7 {
            return Err(PacketError::InvalidPayload("audio payload too short"));
        }
        let mut ch_bytes = [0u8; 2];
        ch_bytes.copy_from_slice(&payload[0..2]);
        let channels = u16::from_be_bytes(ch_bytes);

        let mut sr_bytes = [0u8; 4];
        sr_bytes.copy_from_slice(&payload[2..6]);
        let sample_rate = u32::from_be_bytes(sr_bytes);

        let codec_id = payload[6];
        Ok((channels, sample_rate, codec_id, &payload[7..]))
    }
}
