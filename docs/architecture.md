# RushAudio Architecture

A design reference for implementers in any language.

## Layered Design

RushAudio has four logical layers. Each has a single responsibility and can be implemented independently.

```
┌──────────────────────────────────────────────────┐
│                   Application                     │
│  (your code: capture, playback, mix, record)      │
│                                                    │
│  API calls: send_audio(), recv_audio(),            │
│  connect(), disconnect()                           │
├──────────────────────────────────────────────────┤
│                  Session Layer                     │
│  Connection state machine                          │
│  Handshake negotiation                             │
│  Keepalive timers                                  │
│  Session teardown                                  │
│  Metadata exchange (track info, stream info)       │
├──────────────────────────────────────────────────┤
│                 Audio Layer                        │
│  Codec encode/decode (Opus, PCM, A-law, μ-law)    │
│  Jitter buffer (in-order, adaptive delay)          │
│  FEC encode/decode (XOR parity)                   │
├──────────────────────────────────────────────────┤
│               Network Layer                        │
│  UDP socket (send, recv, bind)                    │
│  Packet serialization (header + payload → bytes)  │
│  Packet deserialization (bytes → header + payload)│
│  Peer addressing (SocketAddr: IP + port)          │
└──────────────────────────────────────────────────┘
```

## Layer Responsibilities

### 1. Network Layer

The only layer that touches the wire.

**Does**:
- Serialize a RushAudio packet struct into raw bytes for the wire
- Deserialize raw bytes into a RushAudio packet struct
- Send bytes to a peer address (UDP)
- Receive bytes from a peer address (UDP)
- Validate magic bytes and minimum length

**Does not**:
- Interpret packet types
- Track sessions
- Encode/decode audio

**Interface** (pseudocode):

```
interface Transport:
    bind(address: string) → Transport
    send(packet: Packet, to: Address) → void
    recv() → (Packet, Address) | None
    local_address() → Address
```

### 2. Audio Layer

Transforms raw PCM samples to/from codec frames.

**Does**:
- Encode PCM → Opus, A-law, μ-law
- Decode Opus, A-law, μ-law → PCM
- Insert packets into jitter buffer by sequence order
- Release packets after target delay
- Adapt target delay to observed jitter
- Generate FEC repair packets from a group
- Recover lost packets using FEC

**Does not**:
- Manage sessions
- Touch the network

**Jitter buffer algorithm** (any language):

```
queue: sorted deque of (seq, timestamp, data, received_ms)

function push(seq, ts, data):
    binary-insert into queue by seq
    if queue.length > max_size: pop_front

function pop():
    if queue.empty(): return None
    head = queue[0]
    age = current_time_ms() - head.received_ms
    if age < target_delay_ms: return None   # not ripe yet
    return pop_front()

function adapt():
    jitter = estimate_jitter()
    target_delay_ms = clamp(jitter * 2 + 10, 20, 400)
```

### 3. Session Layer

Manages the lifetime of a connection with a remote peer.

**Does**:
- Track connection state (disconnected → connecting → connected → streaming → …)
- Build and parse handshake messages
- Detect session timeout (30s of no activity)
- Send periodic keepalives (every 2–5s)
- Clean up stale sessions
- Send/receive stream metadata (track title, artist, codec info, custom fields)

**Does not**:
- Serialize packets
- Process audio data beyond forwarding

**State machine** (any language):

```
enum State { Disconnected, Connecting, Connected, Streaming, Paused }

function on_handshake_request(packet, from_addr):
    if state != Disconnected: return  # ignore
    config = parse_handshake_request(packet)
    if config is valid:
        create_session(from_addr, config)
        send_handshake_response(accepted=true)
        state = Connected

function on_handshake_response(packet, from_addr):
    if state != Connecting: return
    (accepted, ssrc) = parse_handshake_response(packet)
    if accepted:
        state = Connected

function on_keepalive(from_addr):
    session.last_activity = now()

function sweep_stale_sessions(timeout_ms=30000):
    for each session:
        if now() - session.last_activity > timeout_ms:
            destroy_session(session)
```

### 4. Application Layer

Your code. This is where you:
- Capture audio from a microphone or file
- Pass PCM frames down to the audio layer for encoding
- Send encoded packets via the network layer
- Receive packets from the network layer
- Pass them to the audio layer for decoding
- Play decoded PCM to speakers or save to file

## Data Flow

### Sender pipeline

```
[Microphone/file] → PCM samples
    ↓
Audio codec: PCM → encoded frame
    ↓ (optional)
FEC encoder: group frames → repair packet
    ↓
Packet: wrap in header (type=AudioData, seq++, timestamp)
    ↓
Transport: serialize header + payload → UDP datagram
    ↓
Network: socket.sendto(bytes, peer_addr)
```

### Receiver pipeline

```
Network: socket.recvfrom() → bytes, peer_addr
    ↓
Transport: deserialize bytes → Packet (header + payload)
    ↓
Type dispatch:
    AudioData → push to jitter buffer
    FECData   → hold for recovery
    KeepAlive → update last_activity, echo back
    Handshake → update state machine
    Control   → start/stop/pause/resume
    Metadata  → store key-value pairs for stream info
    ↓
Jitter buffer pop() → (timestamp, encoded_frame)
    ↓
Audio codec: decode → PCM samples
    ↓
[Speakers/file] → play PCM
```

## Implementation Strategy

### Minimum viable (any language)

1. Write a `Packet` struct with `encode()` → bytes and `decode(bytes)` → Packet
2. Write a UDP socket wrapper with `send()` and `recv()`
3. Handle HandshakeRequest/Response to establish a session
4. Send/receive AudioData packets
5. Handle KeepAlive

Then add:

6. Jitter buffer for smooth playout
7. FEC for loss recovery
8. Stats for monitoring

### Testing your implementation

| Test case | What to check |
|-----------|---------------|
| Encode then decode a packet | Fields match exactly |
| Packet with wrong magic | Rejected |
| Packet with truncated payload | Rejected |
| Handshake round-trip | Both sides agree on config |
| Audio with all 4 codecs | Decode(encode(pcm)) ≈ pcm |
| FEC: drop 1 of 4 | Recovered payload matches original |
| Jitter buffer: send out-of-order | Packets released in sequence order |
| Keepalive timeout | Session cleaned up after 30s |
| Sequence wrap-around | No gaps after 0xFFFFFFFF → 0x00000000 |
| Metadata encode/decode roundtrip | All key-value pairs survive |
| Metadata custom keys (0x80+) | Custom entries preserved correctly |
| Metadata from/to packet | Packet type and payload intact |

## Protocol vs Implementation

This document describes the RushAudio **protocol** — the wire format and expected behavior. The architecture is designed so that each layer can be implemented independently in any language.

The Rust reference implementation in `src/` follows these same layers and can be used as a working example to validate your implementation against.
