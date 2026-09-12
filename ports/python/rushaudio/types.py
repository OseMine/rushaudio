"""Core protocol types for the RushAudio Python port.

Mirrors ``src/protocol/types.rs`` in the Rust reference implementation.
"""

import enum
from dataclasses import dataclass

from . import constants as _c


class PacketType(enum.IntEnum):
    """Packet type identifiers as defined in the wire spec, section 4."""

    AUDIO_DATA = 0x01
    FEC_DATA = 0x02
    HANDSHAKE_REQUEST = 0x03
    HANDSHAKE_RESPONSE = 0x04
    KEEP_ALIVE = 0x05
    STREAM_CONTROL = 0x06
    STATS_REPORT = 0x07
    SIL = 0x08
    METADATA = 0x09
    AUDIO_LEVEL = 0x0A

    @classmethod
    def from_u8(cls, value: int) -> "PacketType | None":
        try:
            return cls(value)
        except ValueError:
            return None


class AudioCodec(enum.IntEnum):
    """Audio codec identifiers carried in AudioData payloads.

    ID 0x01 is documented as raw Opus. 0x02-0x04 correspond to real G.711
    conversion in the reference implementation (not yet ported here).
    """

    OPUS = 0x01
    RAW_PCM_I16 = 0x02
    PCM_A_LAW = 0x03
    PCM_MU_LAW = 0x04

    @classmethod
    def from_u8(cls, value: int) -> "AudioCodec | None":
        try:
            return cls(value)
        except ValueError:
            return None

    def to_u8(self) -> int:
        return int(self)


@dataclass
class StreamConfig:
    """Desired stream configuration negotiated during handshake."""

    sample_rate: int = _c.DEFAULT_SAMPLE_RATE
    channels: int = _c.OPUS_CHANNELS
    bitrate: int = 128_000
    frame_duration_ms: int = _c.DEFAULT_FRAME_DURATION_MS
    codec: AudioCodec = AudioCodec.OPUS
    ssrc: int = 0


class ConnectionState(enum.Enum):
    """Session state machine states (spec section 5.1)."""

    DISCONNECTED = "Disconnected"
    CONNECTING = "Connecting"
    CONNECTED = "Connected"
    STREAMING = "Streaming"
    PAUSED = "Paused"
    DISCONNECTING = "Disconnecting"


@dataclass
class StreamStats:
    """Cumulative per-connection statistics."""

    packets_sent: int = 0
    packets_received: int = 0
    packets_lost: int = 0
    fec_packets_sent: int = 0
    fec_packets_recovered: int = 0
    bytes_sent: int = 0
    bytes_received: int = 0
    jitter_ms: float = 0.0
    rtt_ms: float = 0.0