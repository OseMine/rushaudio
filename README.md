# RushAudio

**A binary protocol for low-latency audio streaming over UDP.**

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Crates.io](https://img.shields.io/crates/v/rushaudio.svg)](https://crates.io/crates/rushaudio)
[![Documentation](https://docs.rs/rushaudio/badge.svg)](https://docs.rs/rushaudio)

RushAudio is a **specification-first** protocol for streaming live audio with sub-100ms latency. The protocol is simple enough to implement in any language from scratch in a few hundred lines of code.

## Quick facts

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

## Installation

```toml
[dependencies]
rushaudio = "1.0.0"
```

## Features

### Protocol

- **9 packet types**: AudioData, FECData, Handshake, KeepAlive, StreamControl, StatsReport, SIL, Metadata
- **4 audio codecs**: Opus, Raw PCM S16LE, A-law, μ-law
- **XOR FEC**: Forward error correction recovering 1 loss per group
- **Jitter buffer**: Adaptive delay, in-order playout, late packet detection
- **Metadata**: Stream info exchange with built-in and custom key-value pairs

### Metadata

Send and receive stream metadata over the protocol:

```rust
use rushaudio::prelude::*;

// Build metadata with the fluent API
let meta = MetadataBuilder::new()
    .track_title("My Song")
    .artist("Artist Name")
    .album("Album")
    .sample_rate(48000)
    .channels(2)
    .codec_info("Opus")
    .bitrate(128000)
    .custom(0x80, b"app-specific-data".to_vec())
    .build();

// Convert to packet and send
let pkt = meta.to_packet(seq, timestamp);
transport.send_packet(&pkt, peer_addr)?;

// Receive and parse
let received = Metadata::from_packet(&packet)?;
if let Some(title) = received.get_string(META_TRACK_TITLE) {
    println!("Now playing: {title}");
}
```

**Built-in keys**: SSRC, track title, artist, album, genre, sample rate, channels, codec info, bitrate, duration, stream title, stream URL.

**Custom keys**: 0x80–0xFF for application-specific data.

## Specification

The canonical protocol specification is in **one file**:

- [`docs/protocol.md`](docs/protocol.md) — complete wire format, state machine, algorithms

Supporting docs:

- [`docs/architecture.md`](docs/architecture.md) — layered design, implementation strategy
- [`TODO.md`](TODO.md) — future feature ideas

## Reference implementation (Rust)

The `src/` directory contains a working Rust implementation. Use it to:

- Validate your implementation against test vectors
- Test interoperability
- Understand the protocol in concrete code

```bash
cd rushaudio
cargo build                         # compiles, zero warnings
cargo test                          # 24 tests, all pass
cargo run --example server          # start a stream receiver on :4210
cargo run --example client          # stream a test tone to the server
cargo run --example client -- 127.0.0.1:4210 "Song Title" "Artist"
```

### Running the examples

**Terminal 1 — Server**:
```bash
cargo run --example server
```

**Terminal 2 — Client**:
```bash
cargo run --example client
# or with custom metadata:
cargo run --example client -- 127.0.0.1:4210 "My Track" "My Artist"
```

The client plays a 440Hz tone on your speakers while streaming to the server. The server prints received audio frames and metadata.

## Implementing in other languages

The protocol spec ([`docs/protocol.md`](docs/protocol.md)) contains everything you need:

- Exact byte layout for every packet type (with ASCII diagrams)
- Session state machine
- Jitter buffer algorithm (with pseudocode)
- FEC algorithm (with worked example)
- Metadata TLV format with built-in key table
- Implementation checklist

**Minimum implementation**: ~200 lines in most languages.

## Examples

### Python (conceptual)

```python
# See docs/protocol.md for exact byte offsets
import socket

sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
sock.bind(("0.0.0.0", 4210))

while True:
    data, addr = sock.recvfrom(4110)
    if data[0:2] != b"RA":     # magic
        continue
    pkt_type = data[3]
    seq = int.from_bytes(data[4:8], "big")
    ts  = int.from_bytes(data[8:12], "big")
    length = int.from_bytes(data[12:14], "big")
    payload = data[14:14+length]

    if pkt_type == 0x09:  # Metadata
        offset = 0
        while offset < len(payload):
            key = payload[offset]
            val_len = int.from_bytes(payload[offset+1:offset+3], "big")
            value = payload[offset+3:offset+3+val_len]
            offset += 3 + val_len
            print(f"key=0x{key:02x} value={value}")
```

### Go (conceptual)

```go
// See docs/protocol.md for exact byte offsets
type Header struct {
    Magic      [2]byte
    Version    uint8
    Type       uint8
    Sequence   uint32
    Timestamp  uint32
    PayloadLen uint16
}
// binary.Read(conn, binary.BigEndian, &header)
```

## Project structure

```
rushaudio/
├── docs/
│   ├── protocol.md         ← THE SPEC. Start here.
│   └── architecture.md     ← Design overview.
├── src/                    ← Rust reference implementation.
│   ├── protocol/           ← Packet serialization, types, metadata
│   │   ├── constants.rs    ← Protocol constants, metadata keys
│   │   ├── metadata.rs     ← Metadata encode/decode, builder
│   │   ├── packet.rs       ← Packet header, encode/decode
│   │   └── types.rs        ← PacketType, AudioCodec, StreamConfig
│   ├── transport/          ← UDP wrapper, connection tracking
│   ├── session/            ← Handshake, session manager
│   ├── audio/              ← Codecs, jitter buffer
│   └── utils/              ← FEC
├── examples/
│   ├── server.rs           ← Streaming server with metadata handling
│   └── client.rs           ← Streaming client with audio playback
├── tests/                  ← Integration test suite (24 tests)
├── .github/workflows/
│   ├── ci.yml              ← CI: check, test, fmt, clippy
│   └── release.yml         ← Release to crates.io + GitHub
└── Cargo.toml
```

## License

MIT
