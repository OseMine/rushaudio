"""Non-blocking UDP transport wrapper.

Mirrors ``src/transport/udp.rs`` in the Rust reference implementation.
"""

import socket
from typing import Optional, Tuple

from . import constants as _c
from .packet import Packet, PacketError

Address = Tuple[str, int]


def parse_host_port(addr: str) -> Address:
    """Parse ``"host:port"`` into a ``(host, port)`` tuple usable by bind()."""
    host, _, port = addr.rpartition(":")
    if not port:
        raise ValueError(f"invalid address (expected 'host:port'): {addr!r}")
    return (host or "0.0.0.0"), int(port)


def resolve_address(dest) -> Address:
    if isinstance(dest, str):
        return parse_host_port(dest)
    return dest  # already a (host, port) tuple


class UdpTransport:
    """A thin, non-blocking wrapper around a UDP socket."""

    def __init__(self, sock: socket.socket) -> None:
        self._socket = sock

    @classmethod
    def bind(cls, addr: str) -> "UdpTransport":
        sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        sock.bind(parse_host_port(addr))
        sock.setblocking(False)
        return cls(sock)

    def send_packet(self, packet: Packet, dest) -> int:
        return self.send_raw(packet.encode(), dest)

    def send_raw(self, data: bytes, dest) -> int:
        return self._socket.sendto(data, resolve_address(dest))

    def recv_packet(self) -> Optional[Tuple[Packet, Address]]:
        """Return the next valid packet or None if none is immediately available.

        Non-parseable datagrams are skipped, matching the reference transport.
        """
        while True:
            try:
                data, src = self._socket.recvfrom(_c.MAX_PACKET_SIZE)
            except (socket.timeout, BlockingIOError, InterruptedError):
                return None
            except ConnectionResetError:
                return None  # Windows: ICMP port-unreachable surfaces as reset
            try:
                packet = Packet.decode(data)
            except PacketError:
                continue
            return packet, (src[0], src[1])

    def set_recv_timeout(self, timeout: Optional[float]) -> None:
        self._socket.settimeout(timeout)

    def local_addr(self) -> Address:
        return (self._socket.getsockname()[0], self._socket.getsockname()[1])

    def socket(self) -> socket.socket:
        return self._socket

    def flush_read(self) -> int:
        count = 0
        while self.recv_packet() is not None:
            count += 1
        return count

    def close(self) -> None:
        self._socket.close()

    def __enter__(self) -> "UdpTransport":
        return self

    def __exit__(self, *exc) -> None:
        self.close()