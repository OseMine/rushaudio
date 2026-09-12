# RushAudio Python Port — Features

A pure-Python (≥ 3.9, standard library only) implementation of the
[RushAudio](https://github.com/OseMine/rushaudio) v1.0.0 protocol, wire-
compatible with the Rust reference implementation.

| Property | Value |
|----------|-------|
| Transport | UDP (default port 4210) |
| Header size | 14 bytes |
| Max payload | 4096 bytes |
| Dependencies | none (stdlib only) |
| Tests | 37 (stdlib `unittest`) |
| Package | `rushaudio` 1.0.0 on PyPI layout |

## Rust feature support on Python

Checklist of the Rust crate's (`src/`) features and their support in this
port. `[x]` = ported, `[ ]` = not yet ported.

- [x] **Zero runtime dependencies** — stdlib only, same as the Rust crate
- [x] **Public API** — `prelude`, `create_server(addr)` / `create_client()`, `DEFAULT_PORT`
- [x] **Packets** — strict encode/decode with validation errors (too short, bad magic, unknown type, truncated payload)
- [x] **Packet types** — all packet type identifiers (`PacketType`) incl. `AudioLevel`
- [x] **Metadata** — TLV encode/decode, ordered entries, fluent `MetadataBuilder`, round-trip through real packets
- [x] **Metadata keys** — all 12 built-in keys (0x01–0x0C) + custom keys (0x80–0xFF)
- [x] **AudioLevels packet** — `AudioLevels` encode/decode + `to_packet`/`from_packet`
- [x] **Transport** — non-blocking `UdpTransport` (`send_packet`/`recv_packet`/`flush_read`)
- [x] **Connections** — per-peer `Connection` with stats and sequence counter
- [x] **Connection pool** — bounded `ConnectionPool` keyed by remote address
- [x] **Sessions** — `SessionManager` with keepalive generation and stale-session sweeping
- [x] **Handshake** — request/response builders + parsers, role/state tracking, timeout
- [x] **Jitter buffer** — sequence-sorted insertion, adaptive delay, statistics (depth, dropped, late, jitter)
- [x] **FEC** — XOR group encoding and single-loss recovery (`FecEncoder`)

- [ ] **G.711 A-law / μ-law codec conversion** — `AudioCodecManager` (`src/audio/codec.rs`)
- [ ] **Opus codec** — pass-through documented, no encoder coupling (matches Rust behavior)
- [ ] **Level meter** — `LevelMeter::measure` peak/RMS dBFS computation from PCM (`src/audio/level.rs`)
- [ ] **StreamControl helper** — START / STOP / PAUSE / RESUME request builders (constants only)
- [ ] **StatsReport helper** — packets_lost / received / jitter / rtt payload builders (type passthrough only)
- [ ] **SIL helper** — Silence Insertion Descriptor payload builder (type passthrough only)
- [ ] **Example scripts** — `client.py` / `server.py` mirrors of the Rust examples
- [ ] **Session state machine** — full `ConnectionState` transitions helper (states defined, transitions manual)

## Protocol features

### Packet types (all 10 defined in the codebase)

| Value | Python name | Purpose | Implemented |
|-------|-------------|---------|-------------|
| 0x01 | `AUDIO_DATA` | Encoded audio frame (channels + sample rate + codec + data) | ✅ |
| 0x02 | `FEC_DATA` | XOR repair packet (group count + sequences + repair data) | ✅ |
| 0x03 | `HANDSHAKE_REQUEST` | Client initiates session (14-byte payload) | ✅ |
| 0x04 | `HANDSHAKE_RESPONSE` | Server accept/reject + its SSRC (5-byte payload) | ✅ |
| 0x05 | `KEEP_ALIVE` | NAT binding, dead-peer detection, RTT | ✅ |
| 0x06 | `STREAM_CONTROL` | START / STOP / PAUSE / RESUME codes | ⚠ type + codes only |
| 0x07 | `STATS_REPORT` | Loss, jitter, RTT exchange | ⚠ type passthrough |
| 0x08 | `SIL` | Silence Insertion Descriptor | ⚠ type passthrough |
| 0x09 | `METADATA` | TLV key-value stream info | ✅ |
| 0x0A | `AUDIO_LEVEL` | Per-packet VU meter (peak/RMS dBFS) | ✅ |

Unknown or reserved types are dropped by the transport, matching the spec.

### Encode/decode validation

`Packet.decode` raises typed errors mirroring the Rust enum:

| Python error | Rust variant | Trigger |
|--------------|--------------|---------|
| `PacketTooShortError` | `TooShort` | datagram < 14 bytes |
| `BadMagicError` | `BadMagic` | missing "RA" magic |
| `UnknownTypeError` | `UnknownType` | reserved packet type byte |
| `TruncatedPayloadError` | `Truncated` | declared payload exceeds datagram |
| `InvalidPayloadError` | `InvalidPayload` | structure/scope violations (metadata, levels, audio) |

### Metadata

- TLV wire format `[key: u8] [value_len: u16 BE] [value]` with strict truncation checks
- All 12 built-in keys (`META_SSRC` … `META_STREAM_URL`) with typed accessors
  `get_u16 / get_u32 / get_u64 / get_string`
- Custom keys `0x80–0xFF` (builder enforces the boundary)
- Fluent `MetadataBuilder` (ssrc, track_title, artist, album, genre,
  sample_rate, channels, codec_info, bitrate, duration_ms, stream_title,
  stream_url, custom)
- Insertion-order preserved; re-setting a key overwrites in place
- `to_packet` / `from_packet` round-trips through real wire packets

### Sessions

- Handshake request/response builders and parsers (spec §4.3/4.4)
- Role/state tracking (`INITIATOR`/`RESPONDER`,
  `IDLE → WAITING_FOR_RESPONSE → COMPLETED`, timeout detection)
- `SessionManager`: sessions keyed by remote addr, keepalive packet
  generation with per-session sequence numbers, stale-session sweeping
  (accepts seconds or `datetime.timedelta`), streaming/paused state

### Reliability & timing

- **XOR FEC**: `generate_repair` builds a repair packet for a group;
  `try_recover` returns `(lost_seq, payload)` when exactly one packet of the
  group is missing, `None` on multi-loss
- **Jitter buffer**: sequence-ordered insertion, 80ms default target delay,
  duplicate dropping, late-packet detection, EMA jitter estimation,
  `adapt_delay()` → `clamp(jitter × 2 + 10, 20, 400)ms`, stats (depth,
  dropped, inserted, late, jitter)
- **Sequence numbers**: 32-bit wrap-around on `Connection.next_seq` (source
  of keepalive/stream sequence numbers)

### Transport & connections

- Non-blocking `UdpTransport` with `send_packet / send_raw / recv_packet /
  flush_read / set_recv_timeout`, auto-skip of garbage datagrams, Windows
  ICMP-reset tolerance
- `Connection`: per-peer state, `StreamStats`, monotonic-atomic-equivalent
  sequence counter, activity timestamps
- Bounded `ConnectionPool` keyed by remote address

## Parity notes

Deliberate, documented differences from the Rust reference:

- PEP 8 naming (`AUDIO_DATA` vs `AudioData`); wire bytes are identical
- `UdpTransport.bind` parses `"host:port"` and returns a non-blocking socket;
  addresses are passed as `"ip:port"` strings or `(host, port)` tuples
- The jitter buffer's `inserted` stat is actually tracked (the Rust counter
  is incremented nowhere); other stats match
- `SessionManager` addresses may be any hashable (strings in the tests)

## Roadmap (not in MVP)

- G.711 A-law / μ-law encode-decode ([`src/audio/codec.rs`](../../src/audio/codec.rs))
- `LevelMeter` peak/RMS computation from PCM ([`src/audio/level.rs`](../../src/audio/level.rs))
- StreamControl packet builder/parser helpers
- StatsReport packet builder/parser helpers
- `client.py` / `server.py` example scripts mirroring the Rust examples
  (tone generator + socket) — no audio I/O dependency planned
- Optional `pytest` wrapper around the same suite
- `py.typed` already shipped; mypy-strict adoption

## Versioning

Version-tracked to match the crate: `rushaudio 1.0.0` ↔ Rust `1.0.0`.