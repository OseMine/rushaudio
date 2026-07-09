use crate::protocol::types::AudioCodec;

const PCM_I16_SAMPLE_SIZE: usize = 2;

pub struct AudioCodecManager;

impl AudioCodecManager {
    /// Encode PCM frames using the specified codec.
    /// For Opus, this is a pass-through placeholder expecting external encoder.
    /// For Raw PCM, returns the data as-is with frame size validation.
    pub fn encode(
        codec: AudioCodec,
        pcm_data: &[u8],
        _sample_rate: u32,
        channels: u16,
    ) -> Result<Vec<u8>, AudioError> {
        match codec {
            AudioCodec::RawPcmI16 => {
                if pcm_data.is_empty() || pcm_data.len() % (channels as usize * PCM_I16_SAMPLE_SIZE) != 0 {
                    return Err(AudioError::InvalidFrameSize);
                }
                Ok(pcm_data.to_vec())
            }
            AudioCodec::PcmALaw => {
                // Simple A-law placeholder: downsample 16-bit PCM to 8-bit A-law
                let mut encoded = Vec::with_capacity(pcm_data.len() / 2);
                for chunk in pcm_data.chunks(2) {
                    if chunk.len() < 2 {
                        break;
                    }
                    let sample = i16::from_ne_bytes([chunk[0], chunk[1]]);
                    encoded.push(linear_to_ulaw(sample));
                }
                Ok(encoded)
            }
            AudioCodec::PcmMuLaw => {
                // Simple mu-law placeholder
                let mut encoded = Vec::with_capacity(pcm_data.len() / 2);
                for chunk in pcm_data.chunks(2) {
                    if chunk.len() < 2 {
                        break;
                    }
                    let sample = i16::from_ne_bytes([chunk[0], chunk[1]]);
                    encoded.push(linear_to_ulaw(sample));
                }
                Ok(encoded)
            }
            AudioCodec::Opus => {
                // Opus requires external library integration.
                // Return raw PCM as placeholder for now.
                Ok(pcm_data.to_vec())
            }
        }
    }

    /// Decode codec data back to PCM.
    pub fn decode(
        codec: AudioCodec,
        encoded_data: &[u8],
        _sample_rate: u32,
        _channels: u16,
    ) -> Result<Vec<u8>, AudioError> {
        match codec {
            AudioCodec::RawPcmI16 => Ok(encoded_data.to_vec()),
            AudioCodec::PcmALaw | AudioCodec::PcmMuLaw => {
                let mut pcm = Vec::with_capacity(encoded_data.len() * 2);
                for &sample in encoded_data {
                    let linear = ulaw_to_linear(sample);
                    pcm.extend_from_slice(&linear.to_ne_bytes());
                }
                Ok(pcm)
            }
            AudioCodec::Opus => {
                // Placeholder: return encoded data as-is
                // Real implementation would use opus decoder
                Ok(encoded_data.to_vec())
            }
        }
    }

    pub fn frame_size(codec: AudioCodec, sample_rate: u32, channels: u16, duration_ms: u64) -> usize {
        match codec {
            AudioCodec::RawPcmI16 => {
                let samples = (sample_rate as u64 * duration_ms) / 1000;
                (samples * channels as u64 * PCM_I16_SAMPLE_SIZE as u64) as usize
            }
            AudioCodec::PcmALaw | AudioCodec::PcmMuLaw => {
                let samples = (sample_rate as u64 * duration_ms) / 1000;
                (samples * channels as u64) as usize
            }
            AudioCodec::Opus => 960 * channels as usize * PCM_I16_SAMPLE_SIZE,
        }
    }
}

#[derive(Debug, Clone)]
pub enum AudioError {
    InvalidFrameSize,
    UnknownCodec,
    DecodeError(&'static str),
}

impl std::fmt::Display for AudioError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidFrameSize => write!(f, "invalid PCM frame size"),
            Self::UnknownCodec => write!(f, "unknown audio codec"),
            Self::DecodeError(msg) => write!(f, "decode error: {msg}"),
        }
    }
}

impl std::error::Error for AudioError {}

// G.711 mu-law encoding (ITU-T standard algorithm)
fn linear_to_ulaw(sample: i16) -> u8 {
    const BIAS: u16 = 132;
    let sign = if sample < 0 { 0x80 } else { 0x00 };
    let abs = if sample == i16::MIN {
        (i16::MAX as u16) + 1
    } else {
        sample.unsigned_abs() as u16
    };
    let abs = abs + BIAS;
    let seg = match abs {
        0..=255 => 0,
        256..=511 => 1,
        512..=1023 => 2,
        1024..=2047 => 3,
        2048..=4095 => 4,
        4096..=8191 => 5,
        8192..=16383 => 6,
        _ => 7,
    };
    let exp = seg as u16;
    let mant = ((abs >> (exp + 3)) & 0x0F) as u8;
    !(sign | (seg << 4) | mant)
}

fn ulaw_to_linear(ulaw: u8) -> i16 {
    let ulaw = !ulaw;
    let sign = if ulaw & 0x80 != 0 { -1i16 } else { 1i16 };
    let exponent = ((ulaw >> 4) & 0x07) as u16;
    let mantissa = (ulaw & 0x0F) as u16;
    let linear = ((mantissa << 1) + 33) << (exponent + 2);
    (sign as i16) * (linear as i16 - 132)
}
