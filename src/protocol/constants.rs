pub const PROTOCOL_MAGIC: [u8; 2] = [0x52, 0x41]; // "RA"
pub const PROTOCOL_VERSION: u8 = 1;

pub const HEADER_SIZE: usize = 14; // 2(magic) + 1(ver) + 1(type) + 4(seq) + 4(ts) + 2(len)
pub const MAX_PAYLOAD_SIZE: usize = 1024;
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
