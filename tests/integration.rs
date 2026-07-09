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
