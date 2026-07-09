use std::net::SocketAddr;
use std::time::{Duration, Instant};

use rushaudio::prelude::*;

/// RushAudio streaming server example.
/// Listens for client connections, handles handshake, receives audio.
fn main() -> std::io::Result<()> {
    let addr = format!("0.0.0.0:{}", rushaudio::DEFAULT_PORT);
    println!("[Server] Starting RushAudio server on {addr}");

    let mut transport = rushaudio::create_server(&addr)?;
    let mut sessions = SessionManager::new();
    let mut next_cleanup = Instant::now();

    loop {
        // Receive packets
        match transport.recv_packet()? {
            Some((packet, src)) => {
                handle_packet(&mut transport, &mut sessions, packet, src);
            }
            None => {
                std::thread::sleep(Duration::from_millis(1));
            }
        }

        // Periodic cleanup of stale sessions
        if next_cleanup.elapsed() > Duration::from_secs(15) {
            let stale = sessions.stale_sessions(Duration::from_secs(30));
            for addr in stale {
                println!("[Server] Removing stale session: {addr}");
                sessions.remove_session(&addr);
            }
            next_cleanup = Instant::now();
        }
    }
}

fn handle_packet(
    transport: &mut UdpTransport,
    sessions: &mut SessionManager,
    packet: Packet,
    src: SocketAddr,
) {
    match packet.header.packet_type {
        PacketType::HandshakeRequest => {
            if let Some((remote_ssrc, config)) = Handshake::parse_request(&packet) {
                println!(
                    "[Server] Handshake request from {src} (SSRC={remote_ssrc}, {}Hz, {}ch)",
                    config.sample_rate, config.channels
                );

                let local_ssrc = rand_ssrc();
                let response = Handshake::new(HandshakeRole::Responder, local_ssrc, config.clone())
                    .build_response(packet.header.sequence, packet.header.timestamp, true);

                if let Err(e) = transport.send_packet(&response, src) {
                    eprintln!("[Server] Failed to send handshake response: {e}");
                    return;
                }

                sessions.create_session(src, config, remote_ssrc);
                sessions.set_streaming(&src);
                println!("[Server] Session established with {src} (SSRC={remote_ssrc})");
            }
        }

        PacketType::AudioData => {
            if let Some(session) = sessions.get_session_mut(&src) {
                if let Ok((channels, sample_rate, _codec_id, frame_data)) =
                    Packet::decode_audio_payload(&packet.payload)
                {
                    session.connection.mark_activity();
                    session.connection.record_received(packet.total_size());
                    println!(
                        "[Server] Audio frame from {src}: seq={}, ts={}, {} bytes, {}ch/{}Hz",
                        packet.header.sequence,
                        packet.header.timestamp,
                        frame_data.len(),
                        channels,
                        sample_rate
                    );
                }
            } else {
                println!("[Server] Audio from unknown client {src}, dropping");
            }
        }

        PacketType::KeepAlive => {
            sessions.handle_keepalive(&src);
            if let Some(pkt) = sessions.send_keepalive(&src) {
                let _ = transport.send_packet(&pkt, src);
            }
        }

        PacketType::StreamControl => {
            let code = packet.payload.first().copied().unwrap_or(0);
            match code {
                rushaudio::protocol::constants::CONTROL_STREAM_STOP => {
                    println!("[Server] Client {src} stopped stream");
                    sessions.remove_session(&src);
                }
                _ => {}
            }
        }

        _ => {
            println!("[Server] Unhandled packet type from {src}: {:?}", packet.header.packet_type);
        }
    }
}

fn rand_ssrc() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let dur = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    (dur.as_nanos() & 0xFFFF_FFFF) as u32
}
