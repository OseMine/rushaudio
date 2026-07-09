# RushAudio Protocol Specification v1.0

A binary, language-agnostic protocol for low-latency audio streaming over UDP.

## Table of Contents

1. [Transport](#1-transport)
2. [Byte Order](#2-byte-order)
3. [Packet Header](#3-packet-header)
4. [Packet Types](#4-packet-types)
5. [Session Lifecycle](#5-session-lifecycle)
6. [Sequence Numbers](#6-sequence-numbers)
7. [Jitter Buffer Algorithm](#7-jitter-buffer-algorithm)
8. [Forward Error Correction](#8-forward-error-correction)
9. [Port Numbers](#9-port-numbers)
10. [Implementation Checklist](#10-implementation-checklist)

---

## 1. Transport

RushAudio runs exclusively over **UDP**. There is no TCP, no HTTP, no WebSocket framing.

- **Default port**: `4210`
- **Maximum datagram size**: 4110 bytes (14 header + 4096 payload)
- **Recommended MTU-safe size**: ≤ 1200 bytes (stay under typical Ethernet MTU of 1500 minus IP/UDP headers)

---

## 2. Byte Order

All multi-byte integers are **big-endian** (network byte order). There is one exception: PCM I16 audio samples within codec payloads use **little-endian** (native WAV order).

| Type | Size | Endianness |
|------|------|------------|
| u8   | 1    | N/A |
| u16  | 2    | big-endian |
| u32  | 4    | big-endian |
| PCM I16 sample | 2 | little-endian |

---

## 3. Packet Header

Every packet begins with a fixed 14-byte header:

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|     magic      |  version  |    type    |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                       sequence                                |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                       timestamp                                |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|    payload_len                 |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|  payload (payload_len bytes) ...
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

### Field descriptions

| Offset | Size | Name | Description |
|--------|------|------|-------------|
| 0 | 2 | magic | Always `0x52 0x41` (ASCII "RA"). Reject packets with wrong magic. |
| 2 | 1 | version | Protocol version. Currently `0x01`. |
| 3 | 1 | type | Packet type (see §4). |
| 4 | 4 | sequence | Monotonically increasing sequence number. See §6. |
| 8 | 4 | timestamp | Media timestamp in milliseconds (monotonic clock). |
| 12 | 2 | payload_len | Length of payload in bytes. Range: 0–1024. |
| 14 | N | payload | Type-specific payload data. Exactly `payload_len` bytes. |

### Minimum packet size

The smallest valid packet is 14 bytes (header with zero-length payload). Any datagram smaller than 14 bytes MUST be rejected.

---

## 4. Packet Types

### Type table

| Value | Name | Direction | Payload required |
|-------|------|-----------|-----------------|
| 0x01 | AudioData | Bidirectional | Yes |
| 0x02 | FECData | Bidirectional | Yes |
| 0x03 | HandshakeRequest | Client→Server | Yes (12–14 bytes) |
| 0x04 | HandshakeResponse | Server→Client | Yes (5 bytes) |
| 0x05 | KeepAlive | Bidirectional | No (empty) |
| 0x06 | StreamControl | Bidirectional | Yes (1 byte) |
| 0x07 | StatsReport | Bidirectional | Yes (variable) |
| 0x08 | SIL | Bidirectional | Yes (variable) |
| 0x09 | Metadata | Bidirectional | Yes (TLV entries) |

Unknown or reserved type values MUST be silently dropped.

---

### 4.1 AudioData (0x01)

Carries one encoded audio frame.

**Payload format**:

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|         channels              |                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+        sample_rate            |
|                                                               |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|   codec_id    |  frame_data ...
+-+-+-+-+-+-+-+-+
```

| Offset | Size | Field | Description |
|--------|------|-------|-------------|
| 0 | 2 | channels | Number of audio channels (1 = mono, 2 = stereo) |
| 2 | 4 | sample_rate | Sample rate in Hz (e.g. 48000, 44100, 16000) |
| 6 | 1 | codec_id | Codec identifier (see table below) |
| 7 | N | frame_data | Codec-specific encoded audio data |

**Codec identifiers**:

| ID | Codec | Frame data format |
|----|-------|-------------------|
| 0x01 | Opus | Raw Opus packet (as produced by opus_encode) |
| 0x02 | Raw PCM S16LE | Interleaved 16-bit signed little-endian samples |
| 0x03 | A-law | 8-bit ITU-T G.711 A-law samples |
| 0x04 | μ-law | 8-bit ITU-T G.711 μ-law samples |

**PCM channel layout**: Samples are interleaved per frame: `[L, R, L, R, ...]` for stereo, `[M, M, M, ...]` for mono.

---

### 4.2 FECData (0x02)

XOR-based forward error correction repair packet. See §8 for algorithm.

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|  group_count  |  sequence[0] (4 bytes)                        |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|   ...         |  sequence[1] (4 bytes)                        |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|   ...         |  repair_data (variable) ...
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

| Offset | Size | Field | Description |
|--------|------|-------|-------------|
| 0 | 1 | group_count | Number of packets in this FEC group (2–8) |
| 1 | 4×N | sequences | Original sequence numbers of grouped packets |
| 1+4×N | M | repair_data | XOR of all original payloads (size = max payload in group) |

---

### 4.3 HandshakeRequest (0x03)

Sent by the client to initiate a session.

**Payload format**:

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                       ssrc                                    |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                       sample_rate                             |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                       bitrate                                 |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|   codec_id  |   channels |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

| Offset | Size | Field | Description |
|--------|------|-------|-------------|
| 0 | 4 | ssrc | Sender's SSRC (random 32-bit ID) |
| 4 | 4 | sample_rate | Desired sample rate in Hz |
| 8 | 4 | bitrate | Desired bitrate in bits per second |
| 12 | 1 | codec_id | Desired codec (see 4.1 codec table) |
| 13 | 1 | channels | Desired channel count (1 or 2) |

Total payload length: 14 bytes.

---

### 4.4 HandshakeResponse (0x04)

Sent by the server in reply to HandshakeRequest.

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|   status     |  ssrc                                          |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|   ssrc (cont) ...
+-+-+-+-+-+-+-+-+-+-+-+-+
```

| Offset | Size | Field | Description |
|--------|------|-------|-------------|
| 0 | 1 | status | `0x01` = accepted, `0x00` = rejected |
| 1 | 4 | ssrc | Responder's SSRC (random 32-bit ID) |

Total payload length: 5 bytes.

---

### 4.5 KeepAlive (0x05)

Zero-length payload. Used bidirectionally to:
- Maintain UDP NAT/firewall bindings
- Detect dead peers via timeout
- Measure round-trip time (sender records send time, receiver echoes back)

The receiver MUST respond with its own KeepAlive packet.

---

### 4.6 StreamControl (0x06)

```
 0
 0 1 2 3 4 5 6 7
+-+-+-+-+-+-+-+-+
|    code       |
+-+-+-+-+-+-+-+-+
```

| Code | Value | Meaning |
|------|-------|---------|
| START | 0x01 | Begin streaming |
| STOP | 0x02 | End streaming, tear down session |
| PAUSE | 0x03 | Pause stream temporarily |
| RESUME | 0x04 | Resume paused stream |

---

### 4.7 StatsReport (0x07)

Periodic statistics exchange (optional). The sender reports observed network conditions.

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                       packets_lost                            |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                       packets_received                        |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                       jitter (fixed-point)                    |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                       rtt (fixed-point)                       |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

| Offset | Size | Field | Description |
|--------|------|-------|-------------|
| 0 | 4 | packets_lost | Cumulative packets lost |
| 4 | 4 | packets_received | Cumulative packets received |
| 8 | 4 | jitter | Estimated jitter in microseconds (fixed-point u32) |
| 12 | 4 | rtt | Round-trip time in microseconds (fixed-point u32) |

---

### 4.8 SIL (0x08) — Silence Insertion Descriptor

Used for comfort noise, DTMF events, or silence gaps.

| Offset | Size | Field | Description |
|--------|------|-------|-------------|
| 0 | 1 | sil_type | `0x01`=comfort noise, `0x02`=DTMF, `0x03`=silence |
| 1 | 1 | duration_ms | Expected duration in ms |
| 2 | N | parameters | Codec-specific payload |

---

### 4.9 Metadata (0x09)

Carries key-value metadata about the stream. Entries use TLV (Type-Length-Value) encoding.

**Wire format per entry**:

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|     key       |         value_len          |  value ...        |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

| Offset | Size | Field | Description |
|--------|------|-------|-------------|
| 0 | 1 | key | Metadata key identifier |
| 1 | 2 | value_len | Length of value in bytes (big-endian) |
| 3 | N | value | Key-specific value data |

Multiple entries are concatenated. The payload contains zero or more entries.

**Built-in metadata keys** (0x01–0x7F reserved by protocol):

| Key | ID | Value Type | Description |
|-----|----|------------|-------------|
| META_SSRC | 0x01 | u32 BE | Sender's SSRC |
| META_TRACK_TITLE | 0x02 | UTF-8 string | Track or song title |
| META_ARTIST | 0x03 | UTF-8 string | Artist or performer name |
| META_ALBUM | 0x04 | UTF-8 string | Album or collection name |
| META_GENRE | 0x05 | UTF-8 string | Genre or category |
| META_SAMPLE_RATE | 0x06 | u32 BE | Sample rate in Hz |
| META_CHANNELS | 0x07 | u16 BE | Number of audio channels |
| META_CODEC_INFO | 0x08 | UTF-8 string | Codec name or description |
| META_BITRATE | 0x09 | u32 BE | Bitrate in bits per second |
| META_DURATION_MS | 0x0A | u64 BE | Total duration in milliseconds |
| META_STREAM_TITLE | 0x0B | UTF-8 string | Stream or broadcast name |
| META_STREAM_URL | 0x0C | UTF-8 string | Stream URL or source |

**Custom metadata keys** (0x80–0xFF):

Keys in this range are user-defined. Receivers that do not recognize a custom key MUST ignore it gracefully.

**Usage notes**:

- Metadata packets SHOULD be sent after handshake and whenever stream info changes
- Metadata is advisory — receivers MAY silently ignore unknown keys
- Strings MUST be valid UTF-8
- A single Metadata packet MAY contain any number of entries

---

## 5. Session Lifecycle

### 5.1 State machine

```
  DISCONNECTED
      │
      │ send HandshakeRequest
      ▼
  CONNECTING
      │
      │ receive HandshakeResponse(accepted)
      ▼
  CONNECTED
      │
      │ receive/send AudioData
      ▼
  STREAMING ◄──── PAUSED
      │              ▲
      │ pause        │ resume
      ▼              │
  DISCONNECTING
      │
      │ send/receive StreamControl(STOP)
      ▼
  DISCONNECTED
```

### 5.2 Handshake flow

```
Client                          Server
  │                               │
  │  ┌─ HandshakeRequest ──────►  │
  │  │   seq=0, ts=0             │  Validate request
  │  │   ssrc=C1                 │  Check codec compatibility
  │  │   sample_rate=48000       │  Allocate resources
  │  │   bitrate=128000          │
  │  │   codec=Opus              │
  │  │   channels=2              │
  │  │                           │
  │  │ ◄── HandshakeResponse ──┐ │
  │  │   seq=0, ts=0            │ │
  │  │   status=accepted        │ │
  │  │   ssrc=S1                │ │
  │  │                           │ │
  │  ║                           ║ │
  │  ║  AudioData stream begins  ║ │
  │  ║  seq=1,2,3,...            ║ │
  │  ║                           ║ │
  │  ◄─── KeepAlive ──────────►  │ │  (bidirectional, every 2–5s)
```

### 5.3 Timestamps

- The `timestamp` field in AudioData packets is a media clock in **milliseconds**.
- The clock starts at 0 at the beginning of the stream.
- Timestamps MUST increase monotonically.
- For a 20ms frame duration, timestamps increment by 20 each packet.
- There is no requirement for timestamps to match wall clock time.

### 5.4 Keepalive interval

- Peers SHOULD send KeepAlive every **2–5 seconds**.
- A peer SHOULD consider the session dead after **30 seconds** of no activity.
- Implementations SHOULD use a timer to sweep stale sessions.

---

## 6. Sequence Numbers

### 6.1 Properties

- 32-bit unsigned integer
- Starts at 0 for the first packet of each session
- Increments by 1 for each packet sent (of any type, across all types)
- Wraps around from `0xFFFFFFFF` to `0x00000000`

### 6.2 Gap detection

When receiving packets, detect loss by comparing the current sequence with the expected next sequence:

```
expected = last_received + 1
if received > expected:
    gap = received - expected
    # gap packets are considered lost
elif received < expected:
    # possible duplicate or out-of-order
    # check if already received (deduplicate)
```

### 6.3 Out-of-order handling

Receivers MUST handle out-of-order delivery (common over UDP). Insert packets into a jitter buffer sorted by sequence number. See §7.

### 6.4 Wrap-around

Comparison uses modular arithmetic:

```
fn seq_diff(a: u32, b: u32) -> u32 {
    (a.wrapping_sub(b))  // works correctly modulo 2^32
}
```

`seq_diff(received, expected)` gives the number of packets received since `expected`, handling wrap-around correctly.

---

## 7. Jitter Buffer Algorithm

### 7.1 Purpose

The jitter buffer absorbs network delay variation (jitter) by holding packets for a target delay before releasing them to the audio decoder.

### 7.2 Algorithm

```
struct JitterBuffer:
    packets: sorted queue ordered by sequence number
    target_delay_ms: u64  (default: 80ms)
    min_delay_ms: u64     (default: 20ms)
    max_delay_ms: u64     (default: 400ms)

procedure push(seq, ts, data):
    # Insert in sequence order (binary search)
    insert(packets, (seq, ts, data), by_sequence)

    # Cap queue size
    if len(packets) > MAX_CAPACITY:
        pop_front(packets)  # discard oldest

procedure pop() -> (ts, data) or None:
    head = packets[0]
    age_ms = now() - head.received_time_ms

    if age_ms < target_delay_ms:
        return None  # not ready yet

    return pop_front(packets)

procedure adapt():
    # Run periodically (every 100 packets or every second)
    observed_jitter = estimate_jitter()
    # 2x jitter + 10ms headroom
    new_target = max(observed_jitter * 2 + 10, min_delay_ms)
    target_delay_ms = clamp(new_target, min_delay_ms, max_delay_ms)
```

### 7.3 Jitter estimation

Use an exponential moving average:

```
jitter = jitter * 0.875 + latest_delta * 0.125
```

where `latest_delta` is the absolute difference between the expected and actual arrival time of the most recent packet (in ms).

### 7.4 Late packet policy

A packet arriving after its playout time (timestamp < last_playout_timestamp - threshold) is discarded. A typical threshold is 1000 sequence numbers.

---

## 8. Forward Error Correction

### 8.1 Encoding

For a group of N data packets with payloads P₁, P₂, ..., Pₙ:

1. Determine `max_len = max(len(P₁), len(P₂), ..., len(Pₙ))`
2. Create repair buffer R of size max_len, initialized to 0
3. For each payload Pᵢ: `R[j] ^= Pᵢ[j]` for j = 0..len(Pᵢ)
4. Build FEC packet with:
   - `group_count = N`
   - `sequences = [seq₁, seq₂, ..., seqₙ]`
   - `repair_data = R`

### 8.2 Decoding

Given a repair packet R and K available original packets (K = N-1):

1. Parse `group_count` and `sequences` from the FEC packet
2. Identify the missing sequence(s)
3. If more than one packet is missing: **recovery failed**. XOR FEC can only recover a single loss per group.
4. If exactly one is missing:
   ```
   recovered = repair_data
   for each available packet P:
       if P.sequence in sequences:
           for j = 0..len(P.payload):
               recovered[j] ^= P.payload[j]
   ```
5. Truncate `recovered` to the expected payload length (stored in the original packet header, which is unknown — use the length of the smallest available payload as an estimate, or zero-pad)

### 8.3 Worked example

**Group**: packets with seq 10, 11, 12, 13

Payloads:
- seq=10: `[0x01, 0x02, 0x03]`
- seq=11: `[0x04, 0x05, 0x06]`
- seq=12: `[0x07, 0x08, 0x09]`
- seq=13: `[0x0A, 0x0B, 0x0C]`

Repair data:
```
R[0] = 0x01 ^ 0x04 ^ 0x07 ^ 0x0A = 0x00
R[1] = 0x02 ^ 0x05 ^ 0x08 ^ 0x0B = 0x00
R[2] = 0x03 ^ 0x06 ^ 0x09 ^ 0x0C = 0x00
```

If seq=11 is lost, recover:
```
recovered[0] = 0x00 ^ 0x01 ^ 0x07 ^ 0x0A = 0x04
recovered[1] = 0x00 ^ 0x02 ^ 0x08 ^ 0x0B = 0x05
recovered[2] = 0x00 ^ 0x03 ^ 0x09 ^ 0x0C = 0x06
→ [0x04, 0x05, 0x06] ✓
```

### 8.4 Group size

Default group size: **4** data packets per 1 FEC packet (25% overhead).

Applications may change group size based on observed packet loss rate:
- Low loss (<1%): group size 8
- Moderate loss (1–5%): group size 4
- High loss (>5%): group size 2

---

## 9. Port Numbers

| Port | Usage |
|------|-------|
| 4210 | Default RushAudio data stream |
| 4211 | Optional: RTCP-style stats channel |
| Ephemeral | Client-side (kernel-assigned) |

Implementations MUST allow configurable ports.

---

## 10. Implementation Checklist

To implement RushAudio in any language, you need:

### Minimum viable implementation

- [ ] UDP socket: bind, send, receive (non-blocking recommended)
- [ ] 14-byte header: parse all fields in big-endian
- [ ] Packet type dispatch (switch on byte 3)
- [ ] AudioData payload: parse channels, sample_rate, codec_id, frame_data
- [ ] HandshakeRequest build/parse (14-byte payload)
- [ ] HandshakeResponse build/parse (5-byte payload)
- [ ] KeepAlive send/respond (empty payload)
- [ ] Sequence number tracking: detect gaps, detect duplicates

### For production use

- [ ] Jitter buffer (in-order insertion, adaptive delay)
- [ ] FEC encode/decode (XOR recovery)
- [ ] Stale session timeout (30s no-activity cutoff)
- [ ] Configurable port, sample rate, bitrate, codec
- [ ] Stats reporting (packet loss, jitter, RTT)
- [ ] Metadata: TLV encode/decode, built-in keys, custom keys (0x80–0xFF)

### Wire format correspondence table

```
Rust type    → Wire format  →  Any language
─────────────────────────────────────────────────
[u8; 2]      →  2 bytes     →  byte[2]
u8           →  1 byte      →  uint8
u16 (BE)     →  2 bytes     →  uint16 (big-endian)
u32 (BE)     →  4 bytes     →  uint32 (big-endian)
Vec<u8>      →  N bytes     →  byte[N]
PCM i16 (LE) →  2 bytes     →  int16 (little-endian)
```
