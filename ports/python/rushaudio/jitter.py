"""Adaptive jitter buffer (spec section 7).

Mirrors ``src/audio/jitter.rs`` in the Rust reference implementation.

Packets are inserted in sequence order and only released once they have aged
past the target delay. The target delay adapts to observed jitter with an
exponential moving average.
"""

import time
from dataclasses import dataclass
from typing import Deque, List, Optional, Tuple

from . import constants as _c


@dataclass
class _JitterPacket:
    data: bytes
    sequence: int
    timestamp: int
    received_at: float


# Placeholder to keep the public surface explicit in type hints.
JitterPacket = _JitterPacket


@dataclass(frozen=True)
class JitterStats:
    depth: int
    dropped: int
    inserted: int
    late: int
    jitter_ms: float
    target_delay_ms: float


class JitterBuffer:
    """Sequence-ordered buffer that absorbs network jitter."""

    def __init__(self, capacity: int = 256) -> None:
        self._packets: List[_JitterPacket] = []
        self._capacity = capacity
        self._target_delay = _c.DEFAULT_JITTER_BUFFER_MS / 1000.0
        self._min_delay = 0.020
        self._max_delay = 0.400
        self._last_playout_ts: Optional[int] = None
        self._dropped = 0
        self._inserted = 0
        self._late = 0
        self._current_jitter = 0.0

    def with_capacity(self, cap: int) -> "JitterBuffer":
        return JitterBuffer(capacity=cap)

    def push(self, sequence: int, timestamp: int, data: bytes) -> None:
        if len(self._packets) >= self._capacity:
            self._packets.pop(0)
            self._dropped += 1

        packet = _JitterPacket(
            data=bytes(data),
            sequence=sequence,
            timestamp=timestamp,
            received_at=time.monotonic(),
        )

        index = self._insert_index(packet.sequence)
        if index < len(self._packets) and self._packets[index].sequence == sequence:
            return  # duplicate, drop silently (mirrors Rust binary_search hit)

        if self._last_playout_ts is not None:
            if sequence < self._last_playout_ts and (
                self._last_playout_ts - sequence
            ) > 1000:
                self._late += 1
                return

        self._packets.insert(index, packet)
        self._inserted += 1

    def _insert_index(self, sequence: int) -> int:
        lo, hi = 0, len(self._packets)
        while lo < hi:
            mid = (lo + hi) // 2
            if self._packets[mid].sequence < sequence:
                lo = mid + 1
            else:
                hi = mid
        return lo

    def pop(self) -> Optional[Tuple[int, bytes]]:
        if not self._packets:
            return None

        now = time.monotonic()
        head = self._packets[0]
        head_age = now - head.received_at
        if head_age < self._target_delay:
            return None

        packet = self._packets.pop(0)
        self._last_playout_ts = packet.timestamp
        self._current_jitter = self._current_jitter * 0.875 + (head_age * 1000.0) * 0.125
        return packet.timestamp, packet.data

    def peek(self) -> Optional[Tuple[int, int, int]]:
        if not self._packets:
            return None
        head = self._packets[0]
        return head.sequence, head.timestamp, len(head.data)

    def len(self) -> int:
        return len(self._packets)

    def __len__(self) -> int:
        return len(self._packets)

    def is_empty(self) -> bool:
        return not self._packets

    def clear(self) -> None:
        self._packets.clear()

    def stats(self) -> JitterStats:
        return JitterStats(
            depth=len(self._packets),
            dropped=self._dropped,
            inserted=self._inserted,
            late=self._late,
            jitter_ms=self._current_jitter,
            target_delay_ms=self._target_delay * 1000.0,
        )

    def adapt_delay(self) -> None:
        new_target = max(self._current_jitter * 2.0 + 10.0, 20.0) / 1000.0
        self._target_delay = min(max(new_target, self._min_delay), self._max_delay)

    def set_min_delay(self, ms: int) -> None:
        self._min_delay = ms / 1000.0

    def set_max_delay(self, ms: int) -> None:
        self._max_delay = ms / 1000.0