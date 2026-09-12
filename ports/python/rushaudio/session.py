"""Session manager: keepalive generation and stale-session sweeping.

Mirrors ``src/session/manager.rs`` in the Rust reference implementation.
"""

import time
from dataclasses import dataclass, field
from datetime import timedelta
from typing import Dict, Hashable, List, Optional, Union

from . import constants as _c
from .connection import Connection
from .packet import Packet
from .types import ConnectionState, PacketType, StreamConfig


@dataclass
class Session:
    """Server-side record of one active session."""

    connection: Connection
    last_keepalive: float = field(default_factory=time.monotonic)
    remote_ssrc: int = 0


class SessionManager:
    """Tracks sessions per remote address (spec section 5.4)."""

    def __init__(
        self, keepalive_interval_ms: int = _c.DEFAULT_KEEPALIVE_INTERVAL_MS
    ) -> None:
        self._sessions: Dict[Hashable, Session] = {}
        self._keepalive_interval_ms = keepalive_interval_ms

    def create_session(
        self, addr: Hashable, config: StreamConfig, remote_ssrc: int
    ) -> None:
        conn = Connection(remote_addr=addr)
        conn.config = config
        conn.ssrc = remote_ssrc
        conn.set_state(ConnectionState.CONNECTED)
        self._sessions[addr] = Session(connection=conn, remote_ssrc=remote_ssrc)

    def get_session(self, addr: Hashable) -> Optional[Session]:
        return self._sessions.get(addr)

    def remove_session(self, addr: Hashable) -> Optional[Session]:
        return self._sessions.pop(addr, None)

    def has_session(self, addr: Hashable) -> bool:
        return addr in self._sessions

    def send_keepalive(self, addr: Hashable) -> Optional[Packet]:
        session = self._sessions.get(addr)
        if session is None:
            return None
        seq = session.connection.next_seq()
        return Packet.new(PacketType.KEEP_ALIVE, seq, 0, b"")

    def handle_keepalive(self, addr: Hashable) -> None:
        session = self._sessions.get(addr)
        if session is not None:
            session.connection.mark_activity()
            session.last_keepalive = time.monotonic()

    def set_streaming(self, addr: Hashable) -> None:
        session = self._sessions.get(addr)
        if session is not None:
            session.connection.set_state(ConnectionState.STREAMING)

    def set_paused(self, addr: Hashable) -> None:
        session = self._sessions.get(addr)
        if session is not None:
            session.connection.set_state(ConnectionState.PAUSED)

    def stale_sessions(
        self, timeout: Union[float, timedelta]
    ) -> List[Hashable]:
        """Return addrs whose last keepalive is older than ``timeout``.

        ``timeout`` may be seconds (float) or a :class:`datetime.timedelta`.
        """
        if isinstance(timeout, timedelta):
            timeout = timeout.total_seconds()
        now = time.monotonic()
        return [
            addr
            for addr, session in self._sessions.items()
            if now - session.last_keepalive > timeout
        ]

    def session_count(self) -> int:
        return len(self._sessions)

    def __len__(self) -> int:
        return len(self._sessions)

    def all_addrs(self) -> List[Hashable]:
        return list(self._sessions.keys())