pub const PROTOCOL_MAGIC: [u8; 2] = [0x52, 0x41]; // "RA"
pub const PROTOCOL_VERSION: u8 = 1;

pub const HEADER_SIZE: usize = 14; // 2(magic) + 1(ver) + 1(type) + 4(seq) + 4(ts) + 2(len)
pub const MAX_PAYLOAD_SIZE: usize = 4096;
pub const MAX_PACKET_SIZE: usize = HEADER_SIZE + MAX_PAYLOAD_SIZE;

// Default timing
pub const DEFAULT_SAMPLE_RATE: u32 = 48000;
pub const DEFAULT_FRAME_DURATION_MS: u64 = 20;
pub const DEFAULT_JITTER_BUFFER_MS: u64 = 80;
pub const DEFAULT_KEEPALIVE_INTERVAL_MS: u64 = 5000;
pub const DEFAULT_HANDSHAKE_TIMEOUT_MS: u64 = 3000;

// Opus defaults
pub const OPUS_FRAME_SIZE_20MS: u32 = 960; // 48000 * 0.020
pub const OPUS_CHANNELS: u16 = 2;

// FEC
pub const FEC_REDUNDANCY_COUNT: u8 = 1;
pub const FEC_GROUP_SIZE: u8 = 4;

// Control codes
pub const CONTROL_STREAM_START: u8 = 0x01;
pub const CONTROL_STREAM_STOP: u8 = 0x02;
pub const CONTROL_STREAM_PAUSE: u8 = 0x03;
pub const CONTROL_STREAM_RESUME: u8 = 0x04;

// Metadata entry header: 1(key) + 2(length) = 3 bytes
pub const METADATA_ENTRY_HEADER_SIZE: usize = 3;

// Built-in metadata keys (0x00 reserved, 0x01-0x7F reserved by protocol)
pub const META_SSRC: u8 = 0x01;
pub const META_TRACK_TITLE: u8 = 0x02;
pub const META_ARTIST: u8 = 0x03;
pub const META_ALBUM: u8 = 0x04;
pub const META_GENRE: u8 = 0x05;
pub const META_SAMPLE_RATE: u8 = 0x06;
pub const META_CHANNELS: u8 = 0x07;
pub const META_CODEC_INFO: u8 = 0x08;
pub const META_BITRATE: u8 = 0x09;
pub const META_DURATION_MS: u8 = 0x0A;
pub const META_STREAM_TITLE: u8 = 0x0B;
pub const META_STREAM_URL: u8 = 0x0C;

// Custom metadata keys start at 0x80
pub const META_CUSTOM_BASE: u8 = 0x80;

// Audio level / VU meter
// Payload: 4(audio_sequence) + 4(audio_timestamp) + 1(peak) + 1(rms)
pub const AUDIO_LEVELS_PAYLOAD_SIZE: usize = 10;
// Levels are encoded in dBFS with 1 dB per unit. 0 = full scale (0 dBFS),
// -127 = quietest non-silent level. LEVEL_SILENCE (-128) means -infinity dBFS
// (digital silence).
pub const LEVEL_DBFS_MAX: i8 = 0;
pub const LEVEL_DBFS_MIN: i8 = -127;
pub const LEVEL_SILENCE: i8 = i8::MIN;
