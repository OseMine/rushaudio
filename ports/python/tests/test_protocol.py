"""Test suite for the RushAudio Python port.

Python mirror of ``tests/integration.rs`` in the Rust reference
implementation, plus an end-to-end UDP round-trip test. Runs with:

    python -m unittest discover -s tests -v
"""

import time
import unittest

from rushaudio import (
    Connection,
    ConnectionPool,
    FecEncoder,
    Handshake,
    HandshakeRole,
    JitterBuffer,
    Metadata,
    MetadataBuilder,
    Packet,
    PacketTooShortError,
    PacketType,
    SessionManager,
    StreamConfig,
    UdpTransport,
    create_client,
    create_server,
)
from rushaudio.constants import (
    META_ARTIST,
    META_CHANNELS,
    META_SAMPLE_RATE,
    META_SSRC,
    META_STREAM_TITLE,
    META_TRACK_TITLE,
)
from rushaudio.packet import BadMagicError, InvalidPayloadError


class PacketTest(unittest.TestCase):
    def test_packet_encode_decode_audio(self):
        payload = bytes([0xAB]) * 100
        pkt = Packet.new(PacketType.AUDIO_DATA, 42, 1000, payload)
        decoded = Packet.decode(pkt.encode())
        self.assertEqual(decoded.header.packet_type, PacketType.AUDIO_DATA)
        self.assertEqual(decoded.header.sequence, 42)
        self.assertEqual(decoded.header.timestamp, 1000)
        self.assertEqual(decoded.payload, payload)

    def test_packet_encode_decode_handshake(self):
        payload = bytes([0x01, 0x02, 0x03])
        pkt = Packet.new(PacketType.HANDSHAKE_REQUEST, 0, 0, payload)
        decoded = Packet.decode(pkt.encode())
        self.assertEqual(decoded.header.packet_type, PacketType.HANDSHAKE_REQUEST)
        self.assertEqual(decoded.payload, payload)

    def test_packet_short_buffer_fails(self):
        with self.assertRaises(PacketTooShortError):
            Packet.decode(b"\x00\x00")

    def test_packet_bad_magic_fails(self):
        buf = bytearray(b"\x00" * 14)
        buf[0] = 0xFF
        buf[1] = 0xFF
        with self.assertRaises(BadMagicError):
            Packet.decode(bytes(buf))

    def test_packet_truncated_payload_fails(self):
        header = Packet.new(PacketType.AUDIO_DATA, 1, 0, b"\x00" * 100).encode()
        with self.assertRaises(Exception):
            Packet.decode(header[:20])


class HandshakeTest(unittest.TestCase):
    def test_handshake_request_build_parse(self):
        config = StreamConfig()
        hs = Handshake(role=HandshakeRole.INITIATOR, ssrc=12345, config=config)
        pkt = hs.build_request(1, 500)
        self.assertEqual(pkt.header.packet_type, PacketType.HANDSHAKE_REQUEST)

        parsed = Handshake.parse_request(pkt)
        self.assertIsNotNone(parsed)
        ssrc, cfg = parsed
        self.assertEqual(ssrc, 12345)
        self.assertEqual(cfg.sample_rate, 48000)
        self.assertEqual(cfg.channels, 2)

    def test_handshake_response_build_parse(self):
        hs = Handshake(role=HandshakeRole.RESPONDER, ssrc=67890, config=StreamConfig())
        pkt = hs.build_response(2, 1000, True)

        parsed = Handshake.parse_response(pkt)
        self.assertIsNotNone(parsed)
        accepted, ssrc = parsed
        self.assertTrue(accepted)
        self.assertEqual(ssrc, 67890)

    def test_handshake_timed_out_without_start(self):
        hs = Handshake(role=HandshakeRole.INITIATOR, ssrc=1, config=StreamConfig())
        self.assertTrue(hs.timed_out())


class JitterBufferTest(unittest.TestCase):
    def test_jitter_buffer_order(self):
        jb = JitterBuffer()
        jb.push(3, 300, bytes([3]) * 10)
        jb.push(1, 100, bytes([1]) * 10)
        jb.push(2, 200, bytes([2]) * 10)

        first = jb.pop()
        self.assertTrue(first is None or first[0] == 100)

        time.sleep(0.12)
        timestamps = []
        while True:
            out = jb.pop()
            if out is None:
                break
            timestamps.append(out[0])
        self.assertEqual(timestamps, [100, 200, 300])

    def test_jitter_buffer_inserted_count(self):
        jb = JitterBuffer()
        jb.push(3, 300, b"x")
        jb.push(1, 100, b"x")
        jb.push(2, 200, b"x")
        self.assertEqual(jb.stats().inserted, 3)
        self.assertEqual(jb.peek(), (1, 100, 1))


class ConnectionPoolTest(unittest.TestCase):
    def test_connection_pool(self):
        addr = "127.0.0.1:5000"
        pool = ConnectionPool(10)
        conn = Connection(remote_addr=addr)
        self.assertTrue(pool.add(conn))
        self.assertTrue(pool.contains(addr))
        self.assertEqual(pool.len(), 1)
        self.assertIsNotNone(pool.get(addr))

        pool.remove(addr)
        self.assertFalse(pool.contains(addr))

    def test_connection_pool_capacity(self):
        pool = ConnectionPool(1)
        pool.add(Connection(remote_addr="127.0.0.1:1"))
        self.assertFalse(pool.add(Connection(remote_addr="127.0.0.1:2")))
        self.assertIsNone(pool.get("127.0.0.1:2"))


class FecTest(unittest.TestCase):
    def test_fec_recovery_single_loss(self):
        p1 = Packet.new(PacketType.AUDIO_DATA, 1, 100, bytes([0x01, 0x02, 0x03]))
        p2 = Packet.new(PacketType.AUDIO_DATA, 2, 200, bytes([0x04, 0x05, 0x06]))
        p3 = Packet.new(PacketType.AUDIO_DATA, 3, 300, bytes([0x07, 0x08, 0x09]))

        group = [p1, p2, p3]
        repair = FecEncoder.generate_repair(group, 99, 0)
        self.assertIsNotNone(repair)

        recovered = FecEncoder.try_recover(repair, [p1, p3])
        self.assertIsNotNone(recovered)
        seq, data = recovered
        self.assertEqual(seq, 2)
        self.assertEqual(data, bytes([0x04, 0x05, 0x06]))

    def test_fec_multiple_loss_unrecoverable(self):
        p1 = Packet.new(PacketType.AUDIO_DATA, 1, 100, b"\x01\x02\x03")
        p2 = Packet.new(PacketType.AUDIO_DATA, 2, 100, b"\x04\x05\x06")
        group = [p1, p2]
        repair = FecEncoder.generate_repair(group, 99, 0)
        # Both group members are missing from `available` -> cannot recover.
        self.assertIsNone(FecEncoder.try_recover(repair, []))


class SessionManagerTest(unittest.TestCase):
    def test_session_manager(self):
        addr = "127.0.0.1:6000"
        mgr = SessionManager()
        mgr.create_session(addr, StreamConfig(), 42)
        self.assertTrue(mgr.has_session(addr))

        mgr.set_streaming(addr)
        pkt = mgr.send_keepalive(addr)
        self.assertIsNotNone(pkt)
        self.assertEqual(pkt.header.packet_type, PacketType.KEEP_ALIVE)
        self.assertEqual(pkt.header.sequence, 0)

        stale = mgr.stale_sessions(0)
        self.assertIn(addr, stale)

        mgr.remove_session(addr)
        self.assertFalse(mgr.has_session(addr))

    def test_session_keepalive_increments_seq(self):
        addr = "127.0.0.1:6001"
        mgr = SessionManager()
        mgr.create_session(addr, StreamConfig(), 1)
        mgr.send_keepalive(addr)
        pkt = mgr.send_keepalive(addr)
        self.assertEqual(pkt.header.sequence, 1)


class AudioPayloadTest(unittest.TestCase):
    def test_audio_payload_encode_decode(self):
        frame = bytes([0x41]) * 960
        payload = Packet.encode_audio_payload(2, 48000, 0x01, frame)
        self.assertGreater(len(payload), len(frame))

        channels, sample_rate, codec_id, data = Packet.decode_audio_payload(payload)
        self.assertEqual(channels, 2)
        self.assertEqual(sample_rate, 48000)
        self.assertEqual(codec_id, 0x01)
        self.assertEqual(data, frame)

    def test_audio_payload_too_short(self):
        with self.assertRaises(InvalidPayloadError):
            Packet.decode_audio_payload(b"\x00\x01")


class ConnectionTest(unittest.TestCase):
    def test_seq_number_atomic(self):
        conn = Connection(remote_addr="127.0.0.1:7000")
        self.assertEqual(conn.current_seq(), 0)
        self.assertEqual(conn.next_seq(), 0)
        self.assertEqual(conn.current_seq(), 1)

    def test_connection_stats(self):
        conn = Connection(remote_addr="127.0.0.1:7001")
        conn.record_sent(100)
        conn.record_received(50)
        conn.record_loss(2)
        self.assertEqual(conn.stats.packets_sent, 1)
        self.assertEqual(conn.stats.bytes_sent, 100)
        self.assertEqual(conn.stats.packets_received, 1)
        self.assertEqual(conn.stats.packets_lost, 2)


class MetadataTest(unittest.TestCase):
    def test_metadata_encode_decode_roundtrip(self):
        meta = (
            MetadataBuilder()
            .ssrc(12345)
            .track_title("Test Track")
            .artist("Test Artist")
            .sample_rate(48000)
            .channels(2)
            .custom(0x80, b"custom value")
            .build()
        )

        decoded = Metadata.decode(meta.encode())

        self.assertEqual(decoded.get_u32(META_SSRC), 12345)
        self.assertEqual(decoded.get_string(META_TRACK_TITLE), "Test Track")
        self.assertEqual(decoded.get_string(META_ARTIST), "Test Artist")
        self.assertEqual(decoded.get_u32(META_SAMPLE_RATE), 48000)
        self.assertEqual(decoded.get_u16(META_CHANNELS), 2)
        self.assertEqual(decoded.get(0x80), b"custom value")
        self.assertFalse(decoded.is_empty())
        self.assertEqual(decoded.len(), 6)

    def test_metadata_to_from_packet(self):
        meta = MetadataBuilder().track_title("Song").stream_title("My Stream").build()

        pkt = meta.to_packet(10, 200)
        self.assertEqual(pkt.header.packet_type, PacketType.METADATA)
        self.assertEqual(pkt.header.sequence, 10)
        self.assertEqual(pkt.header.timestamp, 200)

        recovered = Metadata.from_packet(pkt)
        self.assertEqual(recovered.get_string(META_TRACK_TITLE), "Song")
        self.assertEqual(recovered.get_string(META_STREAM_TITLE), "My Stream")

    def test_metadata_overwrite(self):
        meta = Metadata()
        meta.set_string(META_TRACK_TITLE, "First")
        self.assertEqual(meta.get_string(META_TRACK_TITLE), "First")

        meta.set_string(META_TRACK_TITLE, "Second")
        self.assertEqual(meta.get_string(META_TRACK_TITLE), "Second")
        self.assertEqual(meta.len(), 1)

    def test_metadata_remove(self):
        meta = Metadata()
        meta.set_string(META_ARTIST, "Artist")
        self.assertTrue(meta.contains(META_ARTIST))

        self.assertTrue(meta.remove(META_ARTIST))
        self.assertFalse(meta.contains(META_ARTIST))
        self.assertTrue(meta.is_empty())

    def test_metadata_empty(self):
        meta = Metadata()
        self.assertTrue(meta.is_empty())
        self.assertEqual(meta.len(), 0)
        self.assertEqual(meta.encode(), b"")
        self.assertTrue(Metadata.decode(b"").is_empty())

    def test_metadata_from_packet_wrong_type(self):
        pkt = Packet.new(PacketType.AUDIO_DATA, 0, 0, b"\x01")
        with self.assertRaises(InvalidPayloadError):
            Metadata.from_packet(pkt)

    def test_metadata_truncated_entry(self):
        data = bytes([0x02, 0x00, 0x0A, ord("A")])
        with self.assertRaises(InvalidPayloadError):
            Metadata.decode(data)

    def test_metadata_truncated_header(self):
        with self.assertRaises(InvalidPayloadError):
            Metadata.decode(bytes([0x02, 0x00]))

    def test_metadata_builder_custom_key(self):
        meta = MetadataBuilder().custom(0x80, b"hello").custom(0xFF, bytes([0x01, 0x02])).build()
        self.assertEqual(meta.get(0x80), b"hello")
        self.assertEqual(meta.get(0xFF), bytes([0x01, 0x02]))
        self.assertEqual(meta.len(), 2)

    def test_metadata_builder_rejects_low_custom_key(self):
        with self.assertRaises(ValueError):
            MetadataBuilder().custom(0x7F, b"x")

    def test_metadata_iter(self):
        meta = MetadataBuilder().track_title("A").artist("B").build()
        collected = [key for key, _ in meta.iter()]
        self.assertEqual(len(collected), 2)

    def test_metadata_packet_wire_format(self):
        meta = MetadataBuilder().track_title("X").build()
        wire = meta.to_packet(0, 0).encode()
        decoded = Packet.decode(wire)
        recovered = Metadata.from_packet(decoded)
        self.assertEqual(recovered.get_string(META_TRACK_TITLE), "X")

    def test_metadata_duration_u64(self):
        meta = MetadataBuilder().duration_ms(123_456_789).build()
        decoded = Metadata.decode(meta.encode())
        self.assertEqual(decoded.get_u64(0x0A), 123_456_789)


class AudioLevelsTest(unittest.TestCase):
    def test_levels_roundtrip(self):
        from rushaudio import AudioLevels

        levels = AudioLevels(
            audio_sequence=5, audio_timestamp=100, peak=-3, rms=-20
        )
        decoded = AudioLevels.decode(levels.encode())
        self.assertEqual(decoded, levels)

    def test_levels_to_packet(self):
        from rushaudio import AudioLevels

        pkt = AudioLevels(1, 2, -1, -1).to_packet(9, 100)
        self.assertEqual(pkt.header.packet_type, PacketType.AUDIO_LEVEL)


class TransportTest(unittest.TestCase):
    def test_udp_roundtrip(self):
        server = create_server("127.0.0.1:0")
        client = create_client()
        server_addr = server.local_addr()
        server_addr_str = f"{server_addr[0]}:{server_addr[1]}"

        try:
            pkt = Packet.new(PacketType.KEEP_ALIVE, 0, 0, b"")
            client.send_packet(pkt, server_addr_str)

            received = None
            deadline = time.monotonic() + 2.0
            while time.monotonic() < deadline:
                received = server.recv_packet()
                if received is not None:
                    break
                time.sleep(0.01)

            self.assertIsNotNone(received)
            recv_pkt, src = received
            self.assertEqual(recv_pkt.header.packet_type, PacketType.KEEP_ALIVE)
            self.assertEqual(recv_pkt.header.sequence, 0)

            server.send_packet(pkt, src)
            back = None
            deadline = time.monotonic() + 2.0
            while time.monotonic() < deadline:
                back = client.recv_packet()
                if back is not None:
                    break
                time.sleep(0.01)
            self.assertIsNotNone(back)
        finally:
            client.close()
            server.close()

    def test_create_server_returns_transport(self):
        server = create_server("0.0.0.0:0")
        self.assertIsInstance(server, UdpTransport)
        server.close()


if __name__ == "__main__":
    unittest.main()