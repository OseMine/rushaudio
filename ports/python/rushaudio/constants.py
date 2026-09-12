"""Protocol constants for the RushAudio Python port.

Mirrors ``src/protocol/constants.rs`` in the Rust reference implementation.
"""

PROTOCOL_MAGIC = bytes((0x52, 0x41))  # ASCII "RA"
PROTOCOL_VERSION = 1

HEADER_SIZE = 14  # 2(magic) + 1(ver) + 1(type) + 4(seq) + 4(ts) + 2(len)
MAX_PAYLOAD_SIZE = 4096
MAX_PACKET_SIZE = HEADER_SIZE + MAX_PAYLOAD_SIZE

# Default timing
DEFAULT_SAMPLE_RATE = 48000
DEFAULT_FRAME_DURATION_MS = 20
DEFAULT_JITTER_BUFFER_MS = 80
DEFAULT_KEEPALIVE_INTERVAL_MS = 5000
DEFAULT_HANDSHAKE_TIMEOUT_MS = 3000

# Opus defaults
OPUS_FRAME_SIZE_20MS = 960  # 48000 * 0.020
OPUS_CHANNELS = 2

# FEC
FEC_REDUNDANCY_COUNT = 1
FEC_GROUP_SIZE = 4

# Control codes
CONTROL_STREAM_START = 0x01
CONTROL_STREAM_STOP = 0x02
CONTROL_STREAM_PAUSE = 0x03
CONTROL_STREAM_RESUME = 0x04

# Metadata entry header: 1(key) + 2(length) = 3 bytes
METADATA_ENTRY_HEADER_SIZE = 3

# Built-in metadata keys (0x00 reserved, 0x01-0x7F reserved by protocol)
META_SSRC = 0x01
META_TRACK_TITLE = 0x02
META_ARTIST = 0x03
META_ALBUM = 0x04
META_GENRE = 0x05
META_SAMPLE_RATE = 0x06
META_CHANNELS = 0x07
META_CODEC_INFO = 0x08
META_BITRATE = 0x09
META_DURATION_MS = 0x0A
META_STREAM_TITLE = 0x0B
META_STREAM_URL = 0x0C

# Custom metadata keys start at 0x80
META_CUSTOM_BASE = 0x80

# Audio level / VU meter
# Payload: 4(audio_sequence) + 4(audio_timestamp) + 1(peak) + 1(rms)
AUDIO_LEVELS_PAYLOAD_SIZE = 10
# Levels are encoded in dBFS with 1 dB per unit. 0 = full scale (0 dBFS),
# -127 = quietest non-silent level. LEVEL_SILENCE (-128) means -infinity dBFS
# (digital silence).
LEVEL_DBFS_MAX = 0
LEVEL_DBFS_MIN = -127
LEVEL_SILENCE = -128