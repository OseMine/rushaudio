"""Session handshake state machine and payload builders/parsers.

Mirrors ``src/session/handshake.rs`` in the Rust reference implementation and
spec section 5.
"""

import enum
import time
from dataclasses import dataclass, field
from typing import Optional, Tuple

from . import constants as _c
from .packet import Packet
from .types import AudioCodec, PacketType, StreamConfig


class HandshakeRole(enum.Enum):
    INITIATOR = "Initiator"
    RESPONDER = "Responder"


class HandshakeState(enum.Enum):
    IDLE = "Idle"
    WAITING_FOR_RESPONSE = "WaitingForResponse"
    WAITING_FOR_REQUEST = "WaitingForRequest"
    COMPLETED = "Completed"
    FAILED = "Failed"


@dataclass
class Handshake:
    """Tracks one side of a handshake and builds/parses its packets."""

    role: HandshakeRole
    ssrc: int
    config: StreamConfig
    state: HandshakeState = HandshakeState.IDLE
    remote_addr: Optional[str] = None
    started_at: Optional[float] = None
    timeout: float = _c.DEFAULT_HANDSHAKE_TIMEOUT_MS / 1000.0

    def build_request(self, seq: int, ts: int) -> Packet:
        """Build a HandshakeRequest packet (14-byte payload, spec 4.3)."""
        payload = (
            self.ssrc.to_bytes(4, "big")
            + self.config.sample_rate.to_bytes(4, "big")
            + self.config.bitrate.to_bytes(4, "big")
            + bytes((self.config.codec.to_u8(), self.config.channels))
        )
        return Packet.new(PacketType.HANDSHAKE_REQUEST, seq, ts, payload)

    def build_response(self, seq: int, ts: int, accepted: bool) -> Packet:
        """Build a HandshakeResponse packet (5-byte payload, spec 4.4)."""
        payload = bytes((0x01 if accepted else 0x00,)) + self.ssrc.to_bytes(4, "big")
        return Packet.new(PacketType.HANDSHAKE_RESPONSE, seq, ts, payload)

    @staticmethod
    def parse_request(packet: Packet) -> Optional[Tuple[int, StreamConfig]]:
        """Parse a HandshakeRequest -> (remote_ssrc, StreamConfig)."""
        payload = packet.payload
        if len(payload) < 12:
            return None

        remote_ssrc = int.from_bytes(payload[0:4], "big")
        sample_rate = int.from_bytes(payload[4:8], "big")
        bitrate = int.from_bytes(payload[8:12], "big")
        codec = AudioCodec.from_u8(payload[12]) if len(payload) > 12 else None
        if codec is None:
            codec = AudioCodec.OPUS
        channels = payload[13] if len(payload) > 13 else 2

        config = StreamConfig(
            sample_rate=sample_rate,
            channels=channels,
            bitrate=bitrate,
            codec=codec,
            ssrc=0,
        )
        return remote_ssrc, config

    @staticmethod
    def parse_response(packet: Packet) -> Optional[Tuple[bool, int]]:
        """Parse a HandshakeResponse -> (accepted, responder_ssrc)."""
        payload = packet.payload
        if len(payload) < 5:
            return None
        accepted = payload[0] == 0x01
        ssrc = int.from_bytes(payload[1:5], "big")
        return accepted, ssrc

    def start(self, remote: str) -> None:
        self.state = HandshakeState.WAITING_FOR_RESPONSE
        self.remote_addr = remote
        self.started_at = time.monotonic()

    def timed_out(self) -> bool:
        if self.started_at is None:
            return True
        return (time.monotonic() - self.started_at) > self.timeout