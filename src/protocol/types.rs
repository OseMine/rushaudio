use crate::protocol::constants::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PacketType {
    AudioData = 0x01,
    FECData = 0x02,
    HandshakeRequest = 0x03,
    HandshakeResponse = 0x04,
    KeepAlive = 0x05,
    StreamControl = 0x06,
    StatsReport = 0x07,
    Sil = 0x08,
    Metadata = 0x09,
}

impl PacketType {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0x01 => Some(Self::AudioData),
            0x02 => Some(Self::FECData),
            0x03 => Some(Self::HandshakeRequest),
            0x04 => Some(Self::HandshakeResponse),
            0x05 => Some(Self::KeepAlive),
            0x06 => Some(Self::StreamControl),
            0x07 => Some(Self::StatsReport),
            0x08 => Some(Self::Sil),
            0x09 => Some(Self::Metadata),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioCodec {
    Opus,
    RawPcmI16,
    PcmALaw,
    PcmMuLaw,
}

impl AudioCodec {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0x01 => Some(Self::Opus),
            0x02 => Some(Self::RawPcmI16),
            0x03 => Some(Self::PcmALaw),
            0x04 => Some(Self::PcmMuLaw),
            _ => None,
        }
    }

    pub fn to_u8(self) -> u8 {
        match self {
            Self::Opus => 0x01,
            Self::RawPcmI16 => 0x02,
            Self::PcmALaw => 0x03,
            Self::PcmMuLaw => 0x04,
        }
    }
}

#[derive(Debug, Clone)]
pub struct StreamConfig {
    pub sample_rate: u32,
    pub channels: u16,
    pub bitrate: u32,
    pub frame_duration_ms: u64,
    pub codec: AudioCodec,
    pub ssrc: u32,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            sample_rate: DEFAULT_SAMPLE_RATE,
            channels: OPUS_CHANNELS,
            bitrate: 128_000,
            frame_duration_ms: DEFAULT_FRAME_DURATION_MS,
            codec: AudioCodec::Opus,
            ssrc: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Streaming,
    Paused,
    Disconnecting,
}

#[derive(Debug, Clone, Copy)]
pub struct StreamStats {
    pub packets_sent: u64,
    pub packets_received: u64,
    pub packets_lost: u64,
    pub fec_packets_sent: u64,
    pub fec_packets_recovered: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub jitter_ms: f64,
    pub rtt_ms: f64,
}

impl Default for StreamStats {
    fn default() -> Self {
        Self {
            packets_sent: 0,
            packets_received: 0,
            packets_lost: 0,
            fec_packets_sent: 0,
            fec_packets_recovered: 0,
            bytes_sent: 0,
            bytes_received: 0,
            jitter_ms: 0.0,
            rtt_ms: 0.0,
        }
    }
}
