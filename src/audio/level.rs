use crate::protocol::constants::{LEVEL_DBFS_MAX, LEVEL_DBFS_MIN, LEVEL_SILENCE};

const PCM_I16_SAMPLE_SIZE: usize = 2;
const I16_FULL_SCALE: f32 = 32768.0;

/// Peak and RMS levels of one PCM frame, encoded in dBFS units.
/// `0` = full scale, negative = quieter, `LEVEL_SILENCE` = digital silence.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameLevels {
    pub peak: i8,
    pub rms: i8,
}

impl FrameLevels {
    pub const fn silence() -> Self {
        Self {
            peak: LEVEL_SILENCE,
            rms: LEVEL_SILENCE,
        }
    }
}

/// Computes per-packet peak and RMS levels from raw PCM frames.
///
/// Measurements must be taken on the uncompressed side (before encoding), so
/// they can be shipped as `AudioLevel` packets and displayed by receivers
/// without decoding audio.
pub struct LevelMeter;

impl LevelMeter {
    /// Measure peak and RMS levels of an interleaved PCM S16LE frame,
    /// aggregated across all channels.
    pub fn measure(pcm: &[u8]) -> FrameLevels {
        let samples = pcm
            .chunks_exact(PCM_I16_SAMPLE_SIZE)
            .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]).unsigned_abs() as f32);

        let mut count = 0usize;
        let mut peak: u16 = 0;
        let mut sum_sq: f64 = 0.0;
        for sample in samples {
            let abs = sample as u16;
            if abs > peak {
                peak = abs;
            }
            sum_sq += (sample as f64) * (sample as f64);
            count += 1;
        }
        if count == 0 {
            return FrameLevels::silence();
        }

        let peak_linear = peak as f32 / I16_FULL_SCALE;
        let rms_linear = ((sum_sq / count as f64) as f32).sqrt() / I16_FULL_SCALE;
        FrameLevels {
            peak: to_dfs_level(peak_linear),
            rms: to_dfs_level(rms_linear),
        }
    }

    /// Measure per-channel peak and RMS levels for interleaved PCM S16LE.
    /// Returns one entry per channel, in channel order (0 = first channel).
    pub fn measure_per_channel(pcm: &[u8], channels: u16) -> Vec<FrameLevels> {
        let channel_count = channels.max(1) as usize;
        let mut peaks = vec![0u16; channel_count];
        let mut sums = vec![0.0f64; channel_count];
        let mut counts = vec![0usize; channel_count];

        for (i, chunk) in pcm.chunks_exact(PCM_I16_SAMPLE_SIZE).enumerate() {
            let ch = i % channel_count;
            let abs = i16::from_le_bytes([chunk[0], chunk[1]]).unsigned_abs() as f32;
            if abs as u16 > peaks[ch] {
                peaks[ch] = abs as u16;
            }
            sums[ch] += (abs as f64) * (abs as f64);
            counts[ch] += 1;
        }

        (0..channel_count)
            .map(|ch| {
                if counts[ch] == 0 {
                    FrameLevels::silence()
                } else {
                    let peak_linear = peaks[ch] as f32 / I16_FULL_SCALE;
                    let rms_linear =
                        ((sums[ch] / counts[ch] as f64) as f32).sqrt() / I16_FULL_SCALE;
                    FrameLevels {
                        peak: to_dfs_level(peak_linear),
                        rms: to_dfs_level(rms_linear),
                    }
                }
            })
            .collect()
    }
}

/// Convert a linear amplitude in [0, 1] to a dBFS integer level.
fn to_dfs_level(linear: f32) -> i8 {
    if linear <= f32::EPSILON {
        return LEVEL_SILENCE;
    }
    let db = 20.0 * linear.log10();
    db.clamp(LEVEL_DBFS_MIN as f32, LEVEL_DBFS_MAX as f32)
        .round() as i8
}
