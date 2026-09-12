"""RushAudio — low-latency audio over IP live streaming protocol.

Pure-Python, zero-dependency port of the Rust reference implementation
(https://github.com/OseMine/rushaudio). Follows the canonical wire spec in
``docs/protocol.md`` of the rushaudio repository.

Public API mirrors the Rust crate:

    import rushaudio
    server = rushaudio.create_server("0.0.0.0:4210")
    client = rushaudio.create_client()

    pkt = Packet.new(PacketType.AUDIO_DATA, seq=1, ts=0,
                     payload=b"...")
    wire = pkt.encode()
    parsed = Packet.decode(wire)
"""

from . import constants as _c
from .connection import Connection, ConnectionPool
from .fec import FecEncoder
from .handshake import Handshake, HandshakeRole, HandshakeState
from .jitter import JitterBuffer, JitterStats
from .levels import AudioLevels
from .metadata import Metadata, MetadataBuilder, MetadataEntry, MetadataMap
from .packet import (
    BadMagicError,
    InvalidPayloadError,
    Packet,
    PacketError,
    PacketHeader,
    PacketTooShortError,
    TruncatedPayloadError,
    UnknownTypeError,
)
from .session import Session, SessionManager
from .transport import UdpTransport
from .types import (
    AudioCodec,
    ConnectionState,
    PacketType,
    StreamConfig,
    StreamStats,
)

VERSION = "1.0.0"
DEFAULT_PORT: int = 4210


def create_server(bind_addr: str) -> UdpTransport:
    """Bind a non-blocking UDP server, e.g. ``create_server("0.0.0.0:4210")``."""
    return UdpTransport.bind(bind_addr)


def create_client() -> UdpTransport:
    """Bind a non-blocking UDP socket on an ephemeral port."""
    return UdpTransport.bind("0.0.0.0:0")


__all__ = [
    "AudioCodec",
    "AudioLevels",
    "BadMagicError",
    "Connection",
    "ConnectionPool",
    "ConnectionState",
    "FecEncoder",
    "Handshake",
    "HandshakeRole",
    "HandshakeState",
    "InvalidPayloadError",
    "JitterBuffer",
    "JitterStats",
    "Metadata",
    "MetadataBuilder",
    "MetadataEntry",
    "MetadataMap",
    "Packet",
    "PacketError",
    "PacketHeader",
    "PacketTooShortError",
    "PacketType",
    "Session",
    "SessionManager",
    "StreamConfig",
    "StreamStats",
    "TruncatedPayloadError",
    "UdpTransport",
    "UnknownTypeError",
    "VERSION",
    "DEFAULT_PORT",
    "create_server",
    "create_client",
]