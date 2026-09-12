"""Packet header and packet encode/decode.

Mirrors ``src/protocol/packet.rs`` in the Rust reference implementation.
"""

from dataclasses import dataclass
from typing import Tuple

from . import constants as _c
from .types import PacketType


class PacketError(Exception):
    """Base class for all packet decode/serialize errors."""


class PacketTooShortError(PacketError):
    """Datagram is smaller than the 14-byte header."""

    def __init__(self, actual: int):
        super().__init__(f"packet too short: {actual} bytes")
        self.actual = actual


class BadMagicError(PacketError):
    """Datagram does not start with the \"RA\" magic bytes."""

    def __init__(self):
        super().__init__("invalid magic bytes")


class UnknownTypeError(PacketError):
    """Datagram carries an unknown/reserved packet type."""

    def __init__(self, packet_type: int):
        super().__init__(f"unknown packet type: 0x{packet_type:02x}")
        self.packet_type = packet_type


class TruncatedPayloadError(PacketError):
    """Header declares a payload larger than the received datagram."""

    def __init__(self, expected: int, actual: int):
        super().__init__(f"truncated payload: expected {expected}, got {actual}")
        self.expected = expected
        self.actual = actual


class InvalidPayloadError(PacketError):
    """Payload failed a structural validation check."""

    def __init__(self, message: str):
        super().__init__(f"invalid payload: {message}")


@dataclass
class PacketHeader:
    """Fixed 14-byte wire header (spec section 3)."""

    magic: bytes
    version: int
    packet_type: PacketType
    sequence: int
    timestamp: int
    payload_len: int


@dataclass
class Packet:
    """A full datagram: header plus raw payload bytes."""

    header: PacketHeader
    payload: bytes

    @classmethod
    def new(
        cls,
        packet_type: PacketType,
        sequence: int,
        timestamp: int,
        payload: bytes,
    ) -> "Packet":
        payload = bytes(payload)
        header = PacketHeader(
            magic=_c.PROTOCOL_MAGIC,
            version=_c.PROTOCOL_VERSION,
            packet_type=PacketType(packet_type),
            sequence=sequence & 0xFFFFFFFF,
            timestamp=timestamp & 0xFFFFFFFF,
            payload_len=len(payload),
        )
        return cls(header, payload)

    def encode(self) -> bytes:
        """Serialize to the wire format.

        Byte order is big-endian for all multi-byte integers.
        """
        h = self.header
        return (
            h.magic
            + bytes((h.version, int(h.packet_type)))
            + h.sequence.to_bytes(4, "big")
            + h.timestamp.to_bytes(4, "big")
            + h.payload_len.to_bytes(2, "big")
            + self.payload
        )

    @classmethod
    def decode(cls, data: bytes) -> "Packet":
        """Parse a datagram, raising :class:`PacketError` on any violation."""
        if len(data) < _c.HEADER_SIZE:
            raise PacketTooShortError(len(data))
        if data[0:2] != _c.PROTOCOL_MAGIC:
            raise BadMagicError()

        version = data[2]
        packet_type = PacketType.from_u8(data[3])
        if packet_type is None:
            raise UnknownTypeError(data[3])

        sequence = int.from_bytes(data[4:8], "big")
        timestamp = int.from_bytes(data[8:12], "big")
        payload_len = int.from_bytes(data[12:14], "big")

        payload_end = _c.HEADER_SIZE + payload_len
        if payload_end > len(data):
            raise TruncatedPayloadError(payload_len, len(data) - _c.HEADER_SIZE)

        return cls(
            PacketHeader(
                magic=data[0:2],
                version=version,
                packet_type=packet_type,
                sequence=sequence,
                timestamp=timestamp,
                payload_len=payload_len,
            ),
            data[_c.HEADER_SIZE:payload_end],
        )

    def total_size(self) -> int:
        return _c.HEADER_SIZE + len(self.payload)

    @staticmethod
    def encode_audio_payload(
        channels: int,
        sample_rate: int,
        codec_id: int,
        frame_data: bytes,
    ) -> bytes:
        """Build an AudioData payload (spec section 4.1)."""
        return (
            int(channels).to_bytes(2, "big")
            + int(sample_rate).to_bytes(4, "big")
            + bytes((codec_id,))
            + bytes(frame_data)
        )

    @staticmethod
    def decode_audio_payload(payload: bytes) -> Tuple[int, int, int, bytes]:
        """Parse an AudioData payload -> (channels, sample_rate, codec_id, frame_data)."""
        if len(payload) < 7:
            raise InvalidPayloadError("audio payload too short")
        return (
            int.from_bytes(payload[0:2], "big"),
            int.from_bytes(payload[2:6], "big"),
            payload[6],
            payload[7:],
        )