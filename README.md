# RushAudio

**A binary protocol for low-latency audio streaming over UDP.**

[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

RushAudio is a **specification-first** protocol for streaming live audio with sub-100ms latency. The protocol is simple enough to implement in any language from scratch in a few hundred lines of code.

## Quick facts

| Property | Value |
|----------|-------|
| Transport | UDP (default port 4210) |
| Header size | 14 bytes |
| Max payload | 1024 bytes |
| Latency target | 30–100ms |
| Loss recovery | XOR FEC (recovers 1 loss per group) |
| Codecs | Opus, PCM S16LE, A-law, μ-law |
| Byte order | Big-endian (except PCM samples: little-endian) |

## Specification

The canonical protocol specification is in **one file**:

- [`docs/protocol.md`](docs/protocol.md) — complete wire format, state machine, algorithms

Read this file if you want to implement RushAudio in any language.

Supporting docs:

- [`docs/architecture.md`](docs/architecture.md) — layered design, implementation strategy

## Reference implementation (Rust)

The `src/` directory contains a working Rust implementation. Use it to:

- Validate your implementation against test vectors
- Test interoperability
- Understand the protocol in concrete code

```bash
cd rushaudio
cargo build                    # compiles, zero warnings
cargo test                     # 13 tests, all pass
cargo run --example server     # start a stream receiver on :4210
cargo run --example client     # stream a test tone to the server
```

## Implementing in other languages

The protocol spec (`docs/protocol.md`) contains everything you need:

- Exact byte layout for every packet type (with ASCII diagrams)
- Session state machine
- Jitter buffer algorithm (with pseudocode)
- FEC algorithm (with worked example)
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
    data, addr = sock.recvfrom(1038)
    if data[0:2] != b"RA":     # magic
        continue
    pkt_type = data[3]
    seq = int.from_bytes(data[4:8], "big")
    ts  = int.from_bytes(data[8:12], "big")
    length = int.from_bytes(data[12:14], "big")
    payload = data[14:14+length]
    # ... handle pkt_type
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
│   ├── protocol/           ← Packet serialization, types
│   ├── transport/          ← UDP wrapper, connection tracking
│   ├── session/            ← Handshake, session manager
│   ├── audio/              ← Codecs, jitter buffer
│   └── utils/              ← FEC
├── examples/               ← Working server and client
└── tests/                  ← Test suite (passes against spec)
```

## License

MIT
