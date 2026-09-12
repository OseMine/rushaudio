"""Per-packet audio level measurement (VU meter data) packet type.

Mirrors ``src/protocol/levels.rs`` in the Rust reference implementation.

Levels are in dBFS with 1 dB per unit: ``0`` = full scale, ``-127`` =
quietest non-silent level, and ``LEVEL_SILENCE`` (-128) = digital silence.
"""

import struct
from dataclasses import dataclass

from . import constants as _c
from .packet import InvalidPayloadError, Packet
from .types import PacketType


@dataclass(frozen=True)
class AudioLevels:
    """Levels describing one ``AudioData`` packet (spec section 4.10)."""

    audio_sequence: int
    audio_timestamp: int
    peak: int
    rms: int

    def encode(self) -> bytes:
        return (
            self.audio_sequence.to_bytes(4, "big")
            + self.audio_timestamp.to_bytes(4, "big")
            + struct.pack("b", self.peak)
            + struct.pack("b", self.rms)
        )

    @classmethod
    def decode(cls, data: bytes) -> "AudioLevels":
        if len(data) < _c.AUDIO_LEVELS_PAYLOAD_SIZE:
            raise InvalidPayloadError("audio level payload too short")
        return cls(
            audio_sequence=int.from_bytes(data[0:4], "big"),
            audio_timestamp=int.from_bytes(data[4:8], "big"),
            peak=struct.unpack("b", data[8:9])[0],
            rms=struct.unpack("b", data[9:10])[0],
        )

    def to_packet(self, seq: int, ts: int) -> Packet:
        return Packet.new(PacketType.AUDIO_LEVEL, seq, ts, self.encode())

    @classmethod
    def from_packet(cls, packet: Packet) -> "AudioLevels":
        if packet.header.packet_type != PacketType.AUDIO_LEVEL:
            raise InvalidPayloadError("not an audio level packet")
        return cls.decode(packet.payload)