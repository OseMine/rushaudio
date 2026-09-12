# RushAudio Features

A binary, specification-first protocol for low-latency audio streaming over UDP, with a zero-dependency Rust reference implementation.

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

## Protocol features

### Packet types

All 10 packet types are defined in the wire spec and implemented in the reference decoder.

| Value | Name | Purpose |
|-------|------|---------|
| 0x01 | AudioData | One encoded audio frame (channels + sample rate + codec + frame data) |
| 0x02 | FECData | XOR repair packet for a group of audio packets |
| 0x03 | HandshakeRequest | Client initiates a session (SSRC + desired stream config) |
| 0x04 | HandshakeResponse | Server accepts/rejects the handshake |
| 0x05 | KeepAlive | NAT binding, dead-peer detection, RTT measurement |
| 0x06 | StreamControl | START / STOP / PAUSE / RESUME |
| 0x07 | StatsReport | Loss, jitter, RTT exchange |
| 0x08 | SIL | Silence Insertion Descriptor (comfort noise, DTMF, silence) |
| 0x09 | Metadata | TLV-encoded key-value stream info |
| 0x0A | AudioLevel | Per-packet peak/RMS levels for VU meters |

Unknown or reserved types are silently dropped.

### Audio codecs

- **Opus** (0x01) — raw Opus packets
- **Raw PCM S16LE** (0x02) — interleaved 16-bit signed little-endian samples
- **A-law** (0x03) — 8-bit ITU-T G.711
- **μ-law** (0x04) — 8-bit ITU-T G.711

### Sessions

- **Handshake**: client sends SSRC, sample rate, bitrate, codec, channels; server replies accepted/rejected + its own SSRC
- **State machine**: `Disconnected → Connecting → Connected → Streaming ↔ Paused`, teardown via StreamControl(STOP)
- **Keepalives**: every 2–5s bidirectionally; sessions stale after 30s are swept
- **Sequence numbers**: 32-bit monotonic per session, modular wrap-around, gap and duplicate detection
- **Metadata exchange**: sent after handshake and on stream-info changes

### Reliability & timing

- **XOR FEC**: one repair packet per N data packets; recovers any single loss per group (default group size 4, 25% overhead)
- **Jitter buffer**: in-order insertion, adaptive target delay (`clamp(jitter × 2 + 10, 20ms, 400ms)` via EMA estimation), late-packet and duplicate handling
- **Timestamps**: 32-bit media clock in milliseconds, monotonically increasing per stream

## Reference implementation (Rust)

The `src/` crate (`rushaudio` 1.1.0) is a working, pure-std implementation of the spec.

- **Zero runtime dependencies** — core protocol uses only the Rust standard library
- **Public API** — `prelude`, `create_server(addr)` / `create_client()` helpers, `DEFAULT_PORT = 4210`
- **Packets** — strict encode/decode with validation errors (too short, bad magic, unknown type, truncated payload)
- **Metadata** — TLV encode/decode, ordered entries, fluent `MetadataBuilder`, round-trip through real packets
- **Codecs** — real G.711 μ-law and A-law conversion; Opus is a documented pass-through awaiting an external encoder
- **Transport** — non-blocking `UdpTransport`, per-peer `Connection` with stats and an atomic sequence counter, bounded `ConnectionPool`
- **Sessions** — `SessionManager` with keepalive generation and stale-session sweeping
- **Jitter buffer** — sequence-sorted insertion, adaptive delay, statistics (depth, dropped, late, jitter)
- **Audio levels** — `AudioLevel` packets (0x0A) carry per-packet peak and RMS in dBFS (1 dB resolution); `LevelMeter` computes both overall and per-channel levels from PCM without any decode on the receiver side
- **FEC** — XOR group encoding and single-loss recovery

### Metadata keys

**Built-in (0x01–0x0C):**

| Key | ID | Type |
|-----|----|------|
| SSRC | 0x01 | u32 |
| Track title | 0x02 | UTF-8 |
| Artist | 0x03 | UTF-8 |
| Album | 0x04 | UTF-8 |
| Genre | 0x05 | UTF-8 |
| Sample rate | 0x06 | u32 |
| Channels | 0x07 | u16 |
| Codec info | 0x08 | UTF-8 |
| Bitrate | 0x09 | u32 |
| Duration (ms) | 0x0A | u64 |
| Stream title | 0x0B | UTF-8 |
| Stream URL | 0x0C | UTF-8 |

**Custom (0x80–0xFF)** — application-defined; unknown keys are gracefully ignored by receivers.

## Examples & demo

- **Server** (`cargo run --example server`) — binds `:4210`, replies to handshakes, sends server metadata (incl. custom key), logs frames and metadata, echoes keepalives, sweeps stale sessions, handles STOP teardown
- **Client** (`cargo run --example client`) — streams a 440Hz test tone as raw PCM on a 20ms cadence, plays it via `cpal`, re-sends dynamic metadata every 5s, sends keepalives every 2s, tears down gracefully
- **Tests** — 32 integration tests covering packet round-trips, handshake, jitter reordering, connection pooling, FEC single-loss recovery, session management, codec conversion, metadata, and audio levels

## Documentation

- `docs/protocol.md` — canonical wire format (byte layouts, state machine, algorithms)
- `docs/architecture.md` — layered design (network / audio / session / application)

## Roadmap

From todos, not yet implemented:

- Stream encryption (AES-GCM / ChaCha20-Poly1305) negotiated during handshake
- Multi-track / multi-stream support with sub-stream IDs
- Congestion control (WebRTC GCC) and bandwidth estimation probes
- Reed-Solomon FEC for multi-loss recovery
- Real-time text / captions channel
- Session migration across IP changes (SSRC + token)
- VBR adaptation and gapless playout markers
- Opus in-band FEC (LBRR)
- NTP timestamp synchronization
- DTMF events (RFC 4733-style)
- Remote stream recording control
- Connection scoring / link-quality reports
- Remote DSP / effects control
- Peer-to-peer relay / SFU mode
- Real Opus codec integration in the Rust crate