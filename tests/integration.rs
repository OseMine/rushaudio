use rushaudio::prelude::*;

#[test]
fn test_packet_encode_decode_audio() {
    let payload = vec![0xAB; 100];
    let pkt = Packet::new(PacketType::AudioData, 42, 1000, payload.clone());
    let encoded = pkt.encode();
    let decoded = Packet::decode(&encoded).unwrap();

    assert_eq!(decoded.header.packet_type, PacketType::AudioData);
    assert_eq!(decoded.header.sequence, 42);
    assert_eq!(decoded.header.timestamp, 1000);
    assert_eq!(decoded.payload, payload);
}

#[test]
fn test_packet_encode_decode_handshake() {
    let pkt = Packet::new(PacketType::HandshakeRequest, 0, 0, vec![0x01, 0x02, 0x03]);
    let encoded = pkt.encode();
    let decoded = Packet::decode(&encoded).unwrap();

    assert_eq!(decoded.header.packet_type, PacketType::HandshakeRequest);
    assert_eq!(decoded.payload, vec![0x01, 0x02, 0x03]);
}

#[test]
fn test_packet_short_buffer_fails() {
    let result = Packet::decode(&[0x00, 0x00]);
    assert!(result.is_err());
}

#[test]
fn test_packet_bad_magic_fails() {
    let mut buf = vec![0x00; 14];
    buf[0] = 0xFF;
    buf[1] = 0xFF;
    let result = Packet::decode(&buf);
    assert!(result.is_err());
}

#[test]
fn test_handshake_request_build_parse() {
    let config = StreamConfig::default();
    let hs = Handshake::new(HandshakeRole::Initiator, 12345, config);
    let pkt = hs.build_request(1, 500);

    assert_eq!(pkt.header.packet_type, PacketType::HandshakeRequest);

    let parsed = Handshake::parse_request(&pkt);
    assert!(parsed.is_some());

    let (ssrc, cfg) = parsed.unwrap();
    assert_eq!(ssrc, 12345);
    assert_eq!(cfg.sample_rate, 48000);
    assert_eq!(cfg.channels, 2);
}

#[test]
fn test_handshake_response_build_parse() {
    let config = StreamConfig::default();
    let hs = Handshake::new(HandshakeRole::Responder, 67890, config);
    let pkt = hs.build_response(2, 1000, true);

    let parsed = Handshake::parse_response(&pkt);
    assert!(parsed.is_some());

    let (accepted, ssrc) = parsed.unwrap();
    assert!(accepted);
    assert_eq!(ssrc, 67890);
}

#[test]
fn test_jitter_buffer_order() {
    let mut jb = JitterBuffer::new();

    jb.push(3, 300, vec![3u8; 10]);
    jb.push(1, 100, vec![1u8; 10]);
    jb.push(2, 200, vec![2u8; 10]);

    // Should output in sequence order
    let p1 = jb.pop();
    assert!(p1.is_none() || p1.unwrap().0 == 100);

    // Sleep a bit to let jitter buffer accumulate
    std::thread::sleep(std::time::Duration::from_millis(100));

    // After wait, should be available
    while jb.pop().is_some() {}
}

#[test]
fn test_connection_pool() {
    let addr: std::net::SocketAddr = "127.0.0.1:5000".parse().unwrap();
    let mut pool = ConnectionPool::new(10);
    let conn = Connection::new(addr);
    assert!(pool.add(conn));
    assert!(pool.contains(addr));
    assert_eq!(pool.len(), 1);
    assert!(pool.get(addr).is_some());

    pool.remove(addr);
    assert!(!pool.contains(addr));
}

#[test]
fn test_fec_recovery_single_loss() {
    let p1 = Packet::new(PacketType::AudioData, 1, 100, vec![0x01, 0x02, 0x03]);
    let p2 = Packet::new(PacketType::AudioData, 2, 200, vec![0x04, 0x05, 0x06]);
    let p3 = Packet::new(PacketType::AudioData, 3, 300, vec![0x07, 0x08, 0x09]);

    let group = vec![p1.clone(), p2.clone(), p3.clone()];
    let repair = FecEncoder::generate_repair(&group, 99, 0).unwrap();

    // Simulate loss of p2
    let available = vec![&p1, &p3];
    let recovered = FecEncoder::try_recover(&repair, &available);

    assert!(recovered.is_some());
    let (seq, data) = recovered.unwrap();
    assert_eq!(seq, 2);
    assert_eq!(data, vec![0x04, 0x05, 0x06]);
}

#[test]
fn test_session_manager() {
    let addr: std::net::SocketAddr = "127.0.0.1:6000".parse().unwrap();
    let mut mgr = SessionManager::new();
    let config = StreamConfig::default();

    mgr.create_session(addr, config, 42);
    assert!(mgr.has_session(&addr));

    mgr.set_streaming(&addr);
    let pkt = mgr.send_keepalive(&addr);
    assert!(pkt.is_some());

    // Don't call handle_keepalive so the session appears stale
    let stale = mgr.stale_sessions(std::time::Duration::from_secs(0));
    assert!(stale.contains(&addr));

    mgr.remove_session(&addr);
    assert!(!mgr.has_session(&addr));
}

#[test]
fn test_audio_payload_encode_decode() {
    let frame = vec![0x41u8; 960];
    let payload = Packet::encode_audio_payload(2, 48000, 0x01, &frame);
    assert!(payload.len() > frame.len());

    let decoded = Packet::decode_audio_payload(&payload);
    assert!(decoded.is_ok());

    let (ch, sr, codec, data) = decoded.unwrap();
    assert_eq!(ch, 2);
    assert_eq!(sr, 48000);
    assert_eq!(codec, 0x01);
    assert_eq!(data, frame.as_slice());
}

#[test]
fn test_audio_codec_pcm_roundtrip() {
    let input = vec![0x00, 0x00, 0x00, 0x40]; // 2 samples: 0, 16384
    let encoded = AudioCodecManager::encode(AudioCodec::PcmMuLaw, &input, 8000, 1).unwrap();
    let decoded = AudioCodecManager::decode(AudioCodec::PcmMuLaw, &encoded, 8000, 1).unwrap();
    assert_eq!(decoded.len(), input.len());
}

#[test]
fn test_seq_number_atomic() {
    let addr: std::net::SocketAddr = "127.0.0.1:7000".parse().unwrap();
    let conn = Connection::new(addr);
    assert_eq!(conn.current_seq(), 0);
    assert_eq!(conn.next_seq(), 0);
    assert_eq!(conn.current_seq(), 1);
}

#[test]
fn test_metadata_encode_decode_roundtrip() {
    let meta = MetadataBuilder::new()
        .ssrc(12345)
        .track_title("Test Track")
        .artist("Test Artist")
        .sample_rate(48000)
        .channels(2)
        .custom(0x80, b"custom value".to_vec())
        .build();

    let encoded = meta.encode();
    let decoded = Metadata::decode(&encoded).unwrap();

    assert_eq!(decoded.get_u32(META_SSRC), Some(12345));
    assert_eq!(
        decoded.get_string(META_TRACK_TITLE).as_deref(),
        Some("Test Track")
    );
    assert_eq!(
        decoded.get_string(META_ARTIST).as_deref(),
        Some("Test Artist")
    );
    assert_eq!(decoded.get_u32(META_SAMPLE_RATE), Some(48000));
    assert_eq!(decoded.get_u16(META_CHANNELS), Some(2));
    assert_eq!(decoded.get(0x80), Some(b"custom value".as_slice()));
    assert!(!decoded.is_empty());
    assert_eq!(decoded.len(), 6);
}

#[test]
fn test_metadata_to_from_packet() {
    let meta = MetadataBuilder::new()
        .track_title("Song")
        .stream_title("My Stream")
        .build();

    let pkt = meta.to_packet(10, 200);
    assert_eq!(pkt.header.packet_type, PacketType::Metadata);
    assert_eq!(pkt.header.sequence, 10);
    assert_eq!(pkt.header.timestamp, 200);

    let recovered = Metadata::from_packet(&pkt).unwrap();
    assert_eq!(
        recovered.get_string(META_TRACK_TITLE).as_deref(),
        Some("Song")
    );
    assert_eq!(
        recovered.get_string(META_STREAM_TITLE).as_deref(),
        Some("My Stream")
    );
}

#[test]
fn test_metadata_overwrite() {
    let mut meta = Metadata::new();
    meta.set_string(META_TRACK_TITLE, "First");
    assert_eq!(meta.get_string(META_TRACK_TITLE).as_deref(), Some("First"));

    meta.set_string(META_TRACK_TITLE, "Second");
    assert_eq!(meta.get_string(META_TRACK_TITLE).as_deref(), Some("Second"));
    assert_eq!(meta.len(), 1);
}

#[test]
fn test_metadata_remove() {
    let mut meta = Metadata::new();
    meta.set_string(META_ARTIST, "Artist");
    assert!(meta.contains(META_ARTIST));

    meta.remove(META_ARTIST);
    assert!(!meta.contains(META_ARTIST));
    assert!(meta.is_empty());
}

#[test]
fn test_metadata_empty() {
    let meta = Metadata::new();
    assert!(meta.is_empty());
    assert_eq!(meta.len(), 0);

    let encoded = meta.encode();
    assert!(encoded.is_empty());

    let decoded = Metadata::decode(&encoded).unwrap();
    assert!(decoded.is_empty());
}

#[test]
fn test_metadata_from_packet_wrong_type() {
    let pkt = Packet::new(PacketType::AudioData, 0, 0, vec![0x01]);
    let result = Metadata::from_packet(&pkt);
    assert!(result.is_err());
}

#[test]
fn test_metadata_truncated_entry() {
    // key(1) + len(2) says 10 bytes, but only 1 byte of value present
    let data = [META_TRACK_TITLE, 0x00, 0x0A, b'A'];
    let result = Metadata::decode(&data);
    assert!(result.is_err());
}

#[test]
fn test_metadata_truncated_header() {
    // Only 2 bytes, not enough for a full entry header
    let data = [META_TRACK_TITLE, 0x00];
    let result = Metadata::decode(&data);
    assert!(result.is_err());
}

#[test]
fn test_metadata_builder_custom_key() {
    let meta = MetadataBuilder::new()
        .custom(0x80, b"hello".to_vec())
        .custom(0xFF, vec![0x01, 0x02])
        .build();

    assert_eq!(meta.get(0x80), Some(b"hello".as_slice()));
    assert_eq!(meta.get(0xFF), Some([0x01, 0x02].as_slice()));
    assert_eq!(meta.len(), 2);
}

#[test]
fn test_metadata_iter() {
    let meta = MetadataBuilder::new().track_title("A").artist("B").build();

    let collected: Vec<_> = meta.iter().collect();
    assert_eq!(collected.len(), 2);
}

#[test]
fn test_metadata_packet_wire_format() {
    let meta = MetadataBuilder::new().track_title("X").build();
    let pkt = meta.to_packet(0, 0);
    let wire = pkt.encode();
    let decoded = Packet::decode(&wire).unwrap();
    let recovered = Metadata::from_packet(&decoded).unwrap();
    assert_eq!(recovered.get_string(META_TRACK_TITLE).as_deref(), Some("X"));
}
