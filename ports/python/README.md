# RushAudio — Python port

A pure-Python, zero-dependency implementation of the [RushAudio](https://github.com/OseMine/rushaudio) protocol: a **binary protocol for low-latency audio streaming over UDP**.

This port mirrors the public API and wire format of the Rust reference implementation, so it is interoperable byte-for-byte on the wire.

| Property | Value |
|----------|-------|
| Transport | UDP (default port 4210) |
| Header size | 14 bytes |
| Max payload | 4096 bytes |
| Latency target | 30–100ms |
| Loss recovery | XOR FEC (recovers 1 loss per group) |
| Codecs | Opus, PCM S16LE, A-law, μ-law |
| Metadata | TLV key-value pairs (built-in + custom) |
| Byte order | Big-endian (except PCM samples: little-endian) |
| Python | ≥ 3.9, standard library only |

## Install

```bash
pip install rushaudio            # from PyPI (once published)
# or build/install from source:
cd ports/python
python -m pip wheel . -w dist    # -> dist/rushaudio-1.0.0-py3-none-any.whl
pip install dist/rushaudio-1.0.0-py3-none-any.whl
```

## Quick start

```python
import rushaudio
from rushaudio.prelude import *

server = rushaudio.create_server("0.0.0.0:4210")   # receiver
client = rushaudio.create_client()                 # sender
peer = "127.0.0.1:4210"

# Encode/decode an audio frame (AudioData payload, spec §4.1)
payload = Packet.encode_audio_payload(2, 48000, AudioCodec.OPUS, opus_bytes)
pkt = Packet.new(PacketType.AUDIO_DATA, seq=1, ts=0, payload=payload)
client.send_packet(pkt, peer)

got = server.recv_packet()          # non-blocking: None if nothing available
if got is not None:
    packet, addr = got
    ch, sr, codec, frame = Packet.decode_audio_payload(packet.payload)
```

## Metadata (TLV)

```python
meta = (
    MetadataBuilder()
    .track_title("My Song")
    .artist("Artist Name")
    .album("Album")
    .sample_rate(48000)
    .channels(2)
    .codec_info("Opus")
    .bitrate(128000)
    .custom(0x80, b"app-specific-data")
    .build()
)
client.send_packet(meta.to_packet(seq=2, ts=20), peer)

received = Metadata.from_packet(packet)          # on the other side
title = received.get_string(META_TRACK_TITLE)    # -> "My Song"
```

## Handshake

```python
config = StreamConfig(sample_rate=48000, channels=2, bitrate=128000, codec=AudioCodec.OPUS)
hs = Handshake(role=HandshakeRole.INITIATOR, ssrc=12345, config=config)
client.send_packet(hs.build_request(0, 0), peer)

# server side:
got = server.recv_packet()
remote_ssrc, cfg = Handshake.parse_request(got[0])
responder = Handshake(role=HandshakeRole.RESPONDER, ssrc=67890, config=cfg)
server.send_packet(responder.build_response(0, 0, accepted=True), got[1])
```

## FEC recovery

```python
group = [pkt1, pkt2, pkt3]                      # original AudioData packets
repair = FecEncoder.generate_repair(group, seq=100, ts=0)
client.send_packet(repair, peer)

# receiver, having lost pkt2 of the group:
recovered = FecEncoder.try_recover(repair, [pkt1, pkt3])   # -> (2, b"...")
```

## Jitter buffer

```python
jb = JitterBuffer()                     # 80ms target delay
jb.push(seq, ts, payload_bytes)         # inserts in sequence order
out = jb.pop()                          # (ts, payload) once aged, else None
jb.adapt_delay()                        # clamp(2*jitter + 10ms, 20ms, 400ms)
stats = jb.stats()                      # depth, dropped, late, jitter
```

## Wire / raw bytes

```python
wire = pkt.encode()                 # header + payload, 14+N bytes
parsed = Packet.decode(wire)        # raises PacketError subclasses on invalid data
```

`UdpTransport.recv_packet()` returns `(Packet, (host, port))` or `None` when
nothing is available (non-blocking). Non-parseable datagrams are skipped.

## Layout

```
ports/python/
├── pyproject.toml
├── rushaudio/
│   ├── __init__.py      # public API: create_server / create_client / DEFAULT_PORT
│   ├── prelude.py       # from rushaudio.prelude import *
│   ├── constants.py     # magic, metadata keys, timing, FEC/level constants
│   ├── types.py         # PacketType, AudioCodec, StreamConfig, StreamStats
│   ├── packet.py        # 14-byte header, Packet encode/decode, errors
│   ├── metadata.py      # Metadata / MetadataBuilder TLV codec
│   ├── levels.py        # AudioLevels (VU meter) packet type
│   ├── fec.py           # XOR FEC encode + single-loss recovery
│   ├── jitter.py        # adaptive jitter buffer
│   ├── handshake.py     # handshake state machine + payload builders
│   ├── transport.py     # non-blocking UdpTransport
│   ├── connection.py    # Connection + bounded ConnectionPool
│   └── session.py       # SessionManager (keepalive, stale sweeping)
└── tests/
    └── test_protocol.py # 37 tests (mirrors tests/integration.rs)
```

## Running the tests

```bash
cd ports/python
python -m unittest discover -s tests -v    # 37 tests, stdlib only
```

## Parity with the Rust reference

| Rust (v1.0.0) | Python |
|----------------|--------|
| `Packet::new / encode / decode` | `Packet.new / encode / decode` |
| `PacketType` | `PacketType` (`AUDIO_DATA`, …) |
| `AudioCodec` | `AudioCodec` (`OPUS`, `RAW_PCM_I16`, …) |
| `Packet::encode_audio_payload / decode_audio_payload` | same names, static methods |
| `MetadataBuilder` fluent API | identical fluent API |
| `Metadata {get,get_string,get_u32,get_u16,encode,decode,to_packet,…}` | identical methods |
| `AudioLevels` | `AudioLevels` |
| `FecEncoder::generate_repair / try_recover` | `FecEncoder.generate_repair / try_recover` |
| `JitterBuffer` | `JitterBuffer` |
| `Handshake` (roles/states, build/parse) | `Handshake` |
| `UdpTransport::bind / send_packet / recv_packet` | `UdpTransport.bind / send_packet / recv_packet` |
| `Connection`, `ConnectionPool` | `Connection`, `ConnectionPool` |
| `SessionManager` | `SessionManager` |
| `prelude` | `rushaudio.prelude` |
| `create_server(addr)` / `create_client()` | identical |

Naming intentionally follows PEP 8 in Python (e.g. `PacketType.AUDIO_DATA`,
`ConnectionState.STREAMING`) while behavior remains wire-identical.

## License

MIT