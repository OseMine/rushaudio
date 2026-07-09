use std::net::{SocketAddr, ToSocketAddrs};
use std::time::{Duration, Instant};

use rushaudio::prelude::*;

/// RushAudio streaming client example.
/// Connects to a server, sends a handshake, then streams audio frames.
fn main() -> std::io::Result<()> {
    let server_addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| format!("127.0.0.1:{}", rushaudio::DEFAULT_PORT));

    let server_socket: SocketAddr = server_addr
        .to_socket_addrs()
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "bad address"))?
        .next()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no address"))?;

    println!("[Client] Connecting to RushAudio server at {server_socket}");

    let mut transport = rushaudio::create_client()?;
    let local_ssrc = rand_ssrc();
    let config = StreamConfig::default();
    let mut handshake = Handshake::new(HandshakeRole::Initiator, local_ssrc, config.clone());
    let mut session_store: Option<SessionManager> = None;
    let mut connected = false;
    let mut seq: u32 = 0;
    let mut audio_seq: u32 = 0;
    let mut last_keepalive = Instant::now();
    let stream_active = true;

    // Initiate handshake
    {
        let hs_pkt = handshake.build_request(seq, 0);
        transport.send_packet(&hs_pkt, server_socket)?;
        println!("[Client] Sent handshake request (SSRC={local_ssrc})");
        handshake.start(server_socket);
        seq += 1;
    }

    let mut last_audio = Instant::now();

    loop {
        // Incoming packets
        if let Some((packet, src)) = transport.recv_packet()? {
            if src != server_socket {
                continue;
            }

            match packet.header.packet_type {
                PacketType::HandshakeResponse => {
                    if let Some((accepted, remote_ssrc)) = Handshake::parse_response(&packet) {
                        if accepted {
                            println!("[Client] Handshake accepted! Remote SSRC={remote_ssrc}");
                            connected = true;

                            let mut mgr = SessionManager::new();
                            mgr.create_session(server_socket, config.clone(), remote_ssrc);
                            mgr.set_streaming(&server_socket);
                            session_store = Some(mgr);
                        } else {
                            eprintln!("[Client] Handshake rejected");
                            break;
                        }
                    }
                }

                PacketType::KeepAlive => {
                    if let Some(ref mut mgr) = session_store {
                        mgr.handle_keepalive(&server_socket);
                    }
                }

                _ => {
                    println!("[Client] Received {:?} from server", packet.header.packet_type);
                }
            }
        }

        // Send audio frames periodically when connected
        if connected && stream_active && last_audio.elapsed() > Duration::from_millis(20) {
            let timestamp = last_audio.elapsed().as_millis() as u32;

            // Generate a simulated audio frame (sine wave placeholder)
            let pcm_frame = generate_test_tone(48000, 2, 20, audio_seq);

            // Encode audio payload
            let audio_payload = Packet::encode_audio_payload(
                2,  // channels
                48000, // sample rate
                0x02, // RawPCMI16 codec id
                &pcm_frame,
            );

            let audio_pkt = Packet::new(PacketType::AudioData, audio_seq, timestamp, audio_payload);
            if let Err(e) = transport.send_packet(&audio_pkt, server_socket) {
                eprintln!("[Client] Send error: {e}");
            }

            if let Some(ref mut mgr) = session_store {
                if let Some(session) = mgr.get_session_mut(&server_socket) {
                    session.connection.record_sent(audio_pkt.total_size());
                }
            }

            audio_seq += 1;
            last_audio = Instant::now();
        }

        // Keepalive
        if connected && last_keepalive.elapsed() > Duration::from_millis(2000) {
            if let Some(ref mut mgr) = session_store {
                if let Some(pkt) = mgr.send_keepalive(&server_socket) {
                    let _ = transport.send_packet(&pkt, server_socket);
                }
            }
            last_keepalive = Instant::now();
        }

        // Heartbeat
        if !connected && handshake.timed_out() {
            eprintln!("[Client] Handshake timed out");
            break;
        }

        std::thread::sleep(Duration::from_millis(1));
    }

    // Send stream stop
    if connected {
        let stop_pkt = Packet::new(
            PacketType::StreamControl,
            seq,
            0,
            vec![rushaudio::protocol::constants::CONTROL_STREAM_STOP],
        );
        let _ = transport.send_packet(&stop_pkt, server_socket);
        println!("[Client] Sent stream stop");
    }

    println!("[Client] Shutting down");
    Ok(())
}

/// Generate a simple sine-wave test tone as PCM I16.
fn generate_test_tone(sample_rate: u32, channels: u16, duration_ms: u64, phase: u32) -> Vec<u8> {
    let samples_per_ch = (sample_rate as u64 * duration_ms) / 1000;
    let total_samples = samples_per_ch * channels as u64;
    let mut buf = Vec::with_capacity(total_samples as usize * 2);

    let freq = 440.0; // A4
    let amplitude: f32 = 0.5;

    for i in 0..total_samples {
        let _ch = (i % channels as u64) as u16;
        let sample_idx = i / channels as u64;
        let t = (sample_idx + phase as u64) as f32 / sample_rate as f32;
        let value = (t * freq * 2.0 * std::f32::consts::PI).sin() * amplitude;
        let sample = (value * i16::MAX as f32) as i16;
        buf.extend_from_slice(&sample.to_ne_bytes());
    }

    buf
}

fn rand_ssrc() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let dur = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    (dur.as_nanos() & 0xFFFF_FFFF) as u32
}
