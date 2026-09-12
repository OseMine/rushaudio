"""Per-peer connection tracking and a bounded connection pool.

Mirrors ``src/transport/connection.rs`` in the Rust reference implementation.
"""

import time
from dataclasses import dataclass, field
from typing import Hashable, Iterator, List, Optional

from .types import ConnectionState, StreamConfig, StreamStats


@dataclass
class Connection:
    """State and statistics for one remote peer."""

    remote_addr: Hashable
    state: ConnectionState = ConnectionState.DISCONNECTED
    config: StreamConfig = field(default_factory=StreamConfig)
    stats: StreamStats = field(default_factory=StreamStats)
    sequence_number: int = 0
    connected_at: Optional[float] = None
    last_activity: float = field(default_factory=time.monotonic)
    ssrc: int = 0

    def next_seq(self) -> int:
        """Return the current sequence number and advance it (mod 2^32)."""
        value = self.sequence_number
        self.sequence_number = (self.sequence_number + 1) & 0xFFFFFFFF
        return value

    def current_seq(self) -> int:
        return self.sequence_number

    def mark_activity(self) -> None:
        self.last_activity = time.monotonic()

    def elapsed_since_activity(self) -> float:
        return time.monotonic() - self.last_activity

    def set_state(self, state: ConnectionState) -> None:
        self.state = state
        if state == ConnectionState.CONNECTED:
            self.connected_at = time.monotonic()

    def record_sent(self, size: int) -> None:
        self.stats.packets_sent += 1
        self.stats.bytes_sent += size

    def record_received(self, size: int) -> None:
        self.stats.packets_received += 1
        self.stats.bytes_received += size

    def record_loss(self, count: int) -> None:
        self.stats.packets_lost += count


class ConnectionPool:
    """A bounded collection of connections keyed by remote address."""

    def __init__(self, max_connections: int) -> None:
        self._connections: List[Connection] = []
        self._max_connections = max_connections

    def get(self, addr: Hashable) -> Optional[Connection]:
        for conn in self._connections:
            if conn.remote_addr == addr:
                return conn
        return None

    def get_mut(self, addr: Hashable) -> Optional[Connection]:
        return self.get(addr)

    def add(self, conn: Connection) -> bool:
        if len(self._connections) >= self._max_connections:
            return False
        self._connections.append(conn)
        return True

    def remove(self, addr: Hashable) -> None:
        self._connections = [c for c in self._connections if c.remote_addr != addr]

    def len(self) -> int:
        return len(self._connections)

    def __len__(self) -> int:
        return len(self._connections)

    def is_empty(self) -> bool:
        return not self._connections

    def iter(self) -> Iterator[Connection]:
        return iter(self._connections)

    def __iter__(self) -> Iterator[Connection]:
        return iter(self._connections)

    def contains(self, addr: Hashable) -> bool:
        return any(c.remote_addr == addr for c in self._connections)