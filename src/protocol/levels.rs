use crate::protocol::constants::*;
use crate::protocol::types::PacketType;
use crate::protocol::{Packet, PacketError};

/// Per-packet audio level measurement (VU meter data).
///
/// Carried in its own packet type (`AudioLevel`) so receivers can drive level
/// meters without decoding the corresponding audio frame (important for
/// encoded codecs such as Opus).
///
/// Levels are in dBFS with 1 dB per unit:
/// - `0`         = full scale (0 dBFS)
/// - `-127`      = quietest non-silent level (-127 dBFS)
/// - `LEVEL_SILENCE` (`i8::MIN`) = digital silence / -infinity dBFS
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioLevels {
    /// Sequence number of the `AudioData` packet these levels describe.
    pub audio_sequence: u32,
    /// Timestamp of the `AudioData` packet these levels describe.
    pub audio_timestamp: u32,
    /// Peak level in dBFS (0 = full scale, negative = quieter).
    pub peak: i8,
    /// RMS level in dBFS (same scale as `peak`).
    pub rms: i8,
}

impl AudioLevels {
    pub fn new(audio_sequence: u32, audio_timestamp: u32, peak: i8, rms: i8) -> Self {
        Self {
            audio_sequence,
            audio_timestamp,
            peak,
            rms,
        }
    }

    /// Encode into a byte vector.
    /// Wire format: [audio_sequence: u32 BE] [audio_timestamp: u32 BE]
    ///              [peak: i8] [rms: i8]
    pub fn encode(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(AUDIO_LEVELS_PAYLOAD_SIZE);
        buf.extend_from_slice(&self.audio_sequence.to_be_bytes());
        buf.extend_from_slice(&self.audio_timestamp.to_be_bytes());
        buf.push(self.peak as u8);
        buf.push(self.rms as u8);
        buf
    }

    /// Decode levels from a byte slice.
    pub fn decode(data: &[u8]) -> Result<Self, PacketError> {
        if data.len() < AUDIO_LEVELS_PAYLOAD_SIZE {
            return Err(PacketError::InvalidPayload("audio level payload too short"));
        }

        let mut seq_bytes = [0u8; 4];
        seq_bytes.copy_from_slice(&data[0..4]);
        let audio_sequence = u32::from_be_bytes(seq_bytes);

        let mut ts_bytes = [0u8; 4];
        ts_bytes.copy_from_slice(&data[4..8]);
        let audio_timestamp = u32::from_be_bytes(ts_bytes);

        Ok(Self {
            audio_sequence,
            audio_timestamp,
            peak: data[8] as i8,
            rms: data[9] as i8,
        })
    }

    /// Wrap into a `Packet` with `PacketType::AudioLevel`.
    pub fn to_packet(&self, seq: u32, ts: u32) -> Packet {
        Packet::new(PacketType::AudioLevel, seq, ts, self.encode())
    }

    /// Extract levels from a `Packet`.
    pub fn from_packet(packet: &Packet) -> Result<Self, PacketError> {
        if packet.header.packet_type != PacketType::AudioLevel {
            return Err(PacketError::InvalidPayload("not an audio level packet"));
        }
        Self::decode(&packet.payload)
    }
}
