"""Metadata TLV encode/decode and fluent builder.

Mirrors ``src/protocol/metadata.rs`` in the Rust reference implementation.

Wire format per entry: ``[key: u8] [value_len: u16 BE] [value: N bytes]``.
Entries preserve insertion order; re-setting an existing key overwrites it
in place rather than appending a duplicate.
"""

from dataclasses import dataclass
from typing import Dict, Iterator, List, Optional, Tuple

from . import constants as _c
from .packet import InvalidPayloadError, Packet
from .types import PacketType


@dataclass
class MetadataEntry:
    """A single key-value metadata entry."""

    key: int
    value: bytes


MetadataMap = Dict[int, bytes]


class Metadata:
    """An ordered collection of metadata entries."""

    def __init__(self, entries: Optional[List[MetadataEntry]] = None) -> None:
        self._entries: List[MetadataEntry] = list(entries or [])

    @classmethod
    def from_map(cls, mapping: MetadataMap) -> "Metadata":
        obj = cls()
        for key, value in mapping.items():
            obj.set(key, value)
        return obj

    def set(self, key: int, value: bytes) -> None:
        value = bytes(value)
        for entry in self._entries:
            if entry.key == key:
                entry.value = value
                return
        self._entries.append(MetadataEntry(key, value))

    def set_string(self, key: int, value: str) -> None:
        self.set(key, value.encode("utf-8"))

    def set_u32(self, key: int, value: int) -> None:
        self.set(key, int(value).to_bytes(4, "big"))

    def set_u16(self, key: int, value: int) -> None:
        self.set(key, int(value).to_bytes(2, "big"))

    def set_u64(self, key: int, value: int) -> None:
        self.set(key, int(value).to_bytes(8, "big"))

    def get(self, key: int) -> Optional[bytes]:
        for entry in self._entries:
            if entry.key == key:
                return entry.value
        return None

    def get_string(self, key: int) -> Optional[str]:
        value = self.get(key)
        if value is None:
            return None
        try:
            return value.decode("utf-8")
        except UnicodeDecodeError:
            return None

    def get_u32(self, key: int) -> Optional[int]:
        value = self.get(key)
        if value is None or len(value) < 4:
            return None
        return int.from_bytes(value[:4], "big")

    def get_u16(self, key: int) -> Optional[int]:
        value = self.get(key)
        if value is None or len(value) < 2:
            return None
        return int.from_bytes(value[:2], "big")

    def get_u64(self, key: int) -> Optional[int]:
        value = self.get(key)
        if value is None or len(value) < 8:
            return None
        return int.from_bytes(value[:8], "big")

    def remove(self, key: int) -> bool:
        before = len(self._entries)
        self._entries = [e for e in self._entries if e.key != key]
        return len(self._entries) < before

    def contains(self, key: int) -> bool:
        return any(e.key == key for e in self._entries)

    def entries(self) -> List[MetadataEntry]:
        return list(self._entries)

    def is_empty(self) -> bool:
        return not self._entries

    def len(self) -> int:
        return len(self._entries)

    def __len__(self) -> int:
        return len(self._entries)

    def iter(self) -> Iterator[Tuple[int, bytes]]:
        for entry in self._entries:
            yield entry.key, entry.value

    def __iter__(self) -> Iterator[Tuple[int, bytes]]:
        return self.iter()

    def encode(self) -> bytes:
        out = bytearray()
        for entry in self._entries:
            out.append(entry.key)
            out += len(entry.value).to_bytes(2, "big")
            out += entry.value
        return bytes(out)

    @classmethod
    def decode(cls, data: bytes) -> "Metadata":
        obj = cls()
        offset = 0
        size = len(data)
        while offset < size:
            if size - offset < _c.METADATA_ENTRY_HEADER_SIZE:
                raise InvalidPayloadError("metadata entry truncated")
            key = data[offset]
            offset += 1
            value_len = int.from_bytes(data[offset:offset + 2], "big")
            offset += 2
            if offset + value_len > size:
                raise InvalidPayloadError("metadata value truncated")
            value = data[offset:offset + value_len]
            offset += value_len
            obj._entries.append(MetadataEntry(key, value))
        return obj

    def to_packet(self, seq: int, ts: int) -> Packet:
        return Packet.new(PacketType.METADATA, seq, ts, self.encode())

    @classmethod
    def from_packet(cls, packet: Packet) -> "Metadata":
        if packet.header.packet_type != PacketType.METADATA:
            raise InvalidPayloadError("not a metadata packet")
        return cls.decode(packet.payload)


class MetadataBuilder:
    """Fluent builder for common stream metadata (Rust ``MetadataBuilder``)."""

    def __init__(self) -> None:
        self._meta = Metadata()

    def _set(self, key: int, value: bytes) -> "MetadataBuilder":
        self._meta.set(key, value)
        return self

    def ssrc(self, value: int) -> "MetadataBuilder":
        return self._set(_c.META_SSRC, int(value).to_bytes(4, "big"))

    def track_title(self, title: str) -> "MetadataBuilder":
        return self._set(_c.META_TRACK_TITLE, title.encode("utf-8"))

    def artist(self, artist: str) -> "MetadataBuilder":
        return self._set(_c.META_ARTIST, artist.encode("utf-8"))

    def album(self, album: str) -> "MetadataBuilder":
        return self._set(_c.META_ALBUM, album.encode("utf-8"))

    def genre(self, genre: str) -> "MetadataBuilder":
        return self._set(_c.META_GENRE, genre.encode("utf-8"))

    def sample_rate(self, rate: int) -> "MetadataBuilder":
        return self._set(_c.META_SAMPLE_RATE, int(rate).to_bytes(4, "big"))

    def channels(self, channels: int) -> "MetadataBuilder":
        return self._set(_c.META_CHANNELS, int(channels).to_bytes(2, "big"))

    def codec_info(self, info: str) -> "MetadataBuilder":
        return self._set(_c.META_CODEC_INFO, info.encode("utf-8"))

    def bitrate(self, bitrate: int) -> "MetadataBuilder":
        return self._set(_c.META_BITRATE, int(bitrate).to_bytes(4, "big"))

    def duration_ms(self, duration: int) -> "MetadataBuilder":
        return self._set(_c.META_DURATION_MS, int(duration).to_bytes(8, "big"))

    def stream_title(self, title: str) -> "MetadataBuilder":
        return self._set(_c.META_STREAM_TITLE, title.encode("utf-8"))

    def stream_url(self, url: str) -> "MetadataBuilder":
        return self._set(_c.META_STREAM_URL, url.encode("utf-8"))

    def custom(self, key: int, value: bytes) -> "MetadataBuilder":
        if key < _c.META_CUSTOM_BASE:
            raise ValueError("custom metadata key must be >= 0x80")
        return self._set(key, value)

    def build(self) -> Metadata:
        return self._meta