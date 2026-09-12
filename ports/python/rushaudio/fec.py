"""XOR-based forward error correction.

Mirrors ``src/utils/fec.rs`` in the Rust reference implementation and
spec section 8.

- Encoding: XOR-reduce the payloads of a group of packets into one repair
  packet that also carries the group count and the original sequence numbers.
- Decoding: XOR the repair data with every surviving packet of the group to
  recover a single missing packet. More than one loss per group is fatal.
"""

from typing import List, Optional, Sequence, Tuple

from .packet import Packet
from .types import PacketType


class FecEncoder:
    """Static helpers for FEC repair packet generation and recovery."""

    @staticmethod
    def generate_repair(group: Sequence[Packet], seq: int, ts: int) -> Optional[Packet]:
        """Build a FECData repair packet for a group of data packets.

        XORs all group payloads into ``repair_data`` (sized to the longest
        payload in the group) and prefixes:
        ``[group_count: u8] [sequence × group_count: u32 BE each]``.
        """
        if not group:
            return None

        max_len = max(len(pkt.payload) for pkt in group)
        repair = bytearray(max_len)
        for pkt in group:
            for index, byte in enumerate(pkt.payload):
                repair[index] ^= byte

        fec_meta = bytearray([len(group)])
        for pkt in group:
            fec_meta += pkt.header.sequence.to_bytes(4, "big")

        return Packet.new(
            PacketType.FEC_DATA, seq, ts, bytes(fec_meta) + bytes(repair)
        )

    @staticmethod
    def try_recover(
        repair_packet: Packet, available: Sequence[Packet]
    ) -> Optional[Tuple[int, bytes]]:
        """Attempt to recover a lost packet.

        Returns ``(lost_sequence, recovered_payload)`` when exactly one
        packet of the FEC group is missing and recoverable; otherwise None.
        """
        payload = repair_packet.payload
        if not payload:
            return None

        group_count = payload[0]
        meta_end = 1 + group_count * 4
        if len(payload) < meta_end:
            return None

        sequences = [
            int.from_bytes(payload[1 + i * 4:5 + i * 4], "big")
            for i in range(group_count)
        ]
        repair_data = bytearray(payload[meta_end:])

        available_seqs = {pkt.header.sequence for pkt in available}
        missing = [s for s in sequences if s not in available_seqs]
        if len(missing) != 1:
            return None
        lost_seq = missing[0]

        for pkt in available:
            if pkt.header.sequence == lost_seq:
                continue
            if pkt.header.sequence in sequences:
                for index, byte in enumerate(pkt.payload):
                    if index < len(repair_data):
                        repair_data[index] ^= byte

        return lost_seq, bytes(repair_data)