use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use rushaudio::prelude::*;

/// Per-client state including received metadata.
struct ClientState {
    metadata: Metadata,
    _remote_ssrc: u32,
}

/// RushAudio streaming server example.
/// Listens for client connections, handles handshake, receives audio and metadata.
fn main() -> std::io::Result<()> {
    let addr = format!("0.0.0.0:{}", rushaudio::DEFAULT_PORT);
    println!("[Server] Starting RushAudio server on {addr}");

    let mut transport = rushaudio::create_server(&addr)?;
    let mut session_mgr = SessionManager::new();
    let mut clients: HashMap<SocketAddr, ClientState> = HashMap::new();
    let mut next_cleanup = Instant::now();

    loop {
        match transport.recv_packet()? {
            Some((packet, src)) => {
                handle_packet(&mut transport, &mut session_mgr, &mut clients, packet, src);
            }
            None => {
                std::thread::sleep(Duration::from_millis(1));
            }
        }

        if next_cleanup.elapsed() > Duration::from_secs(15) {
            let stale = session_mgr.stale_sessions(Duration::from_secs(30));
            for addr in stale {
                println!("[Server] Removing stale session: {addr}");
                session_mgr.remove_session(&addr);
                clients.remove(&addr);
            }
            next_cleanup = Instant::now();
        }
    }
}

fn handle_packet(
    transport: &mut UdpTransport,
    sessions: &mut SessionManager,
    clients: &mut HashMap<SocketAddr, ClientState>,
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

                clients.insert(
                    src,
                    ClientState {
                        metadata: Metadata::new(),
                        _remote_ssrc: remote_ssrc,
                    },
                );

                println!("[Server] Session established with {src} (SSRC={remote_ssrc})");

                // Respond with server metadata
                let server_meta = MetadataBuilder::new()
                    .ssrc(local_ssrc)
                    .stream_title("RushAudio Demo Server")
                    .stream_url("http://localhost:4210")
                    .codec_info("RawPCMI16 / Opus")
                    .custom(0x80, b"server-v1".to_vec())
                    .build();
                let meta_pkt = server_meta.to_packet(0, 0);
                if let Err(e) = transport.send_packet(&meta_pkt, src) {
                    eprintln!("[Server] Failed to send server metadata: {e}");
                } else {
                    println!("[Server] Sent server metadata to {src}");
                }
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
                        "[Server] Audio from {src}: seq={}, {} bytes, {}ch/{}Hz",
                        packet.header.sequence,
                        frame_data.len(),
                        channels,
                        sample_rate
                    );
                }
            } else {
                println!("[Server] Audio from unknown client {src}, dropping");
            }
        }

        PacketType::Metadata => match Metadata::from_packet(&packet) {
            Ok(meta) => {
                if let Some(client) = clients.get_mut(&src) {
                    client.metadata = meta.clone();
                }

                println!("[Server] Metadata from {src}:");
                print_metadata(&meta);
            }
            Err(e) => {
                eprintln!("[Server] Failed to decode metadata from {src}: {e}");
            }
        },

        PacketType::KeepAlive => {
            sessions.handle_keepalive(&src);
            if let Some(pkt) = sessions.send_keepalive(&src) {
                let _ = transport.send_packet(&pkt, src);
            }
        }

        PacketType::StreamControl => {
            let code = packet.payload.first().copied().unwrap_or(0);
            match code {
                CONTROL_STREAM_STOP => {
                    println!("[Server] Client {src} stopped stream");
                    clients.remove(&src);
                    sessions.remove_session(&src);
                }
                _ => {}
            }
        }

        _ => {
            println!(
                "[Server] Unhandled packet type from {src}: {:?}",
                packet.header.packet_type
            );
        }
    }
}

fn print_metadata(meta: &Metadata) {
    if let Some(title) = meta.get_string(META_TRACK_TITLE) {
        println!("    Track: {title}");
    }
    if let Some(artist) = meta.get_string(META_ARTIST) {
        println!("    Artist: {artist}");
    }
    if let Some(album) = meta.get_string(META_ALBUM) {
        println!("    Album: {album}");
    }
    if let Some(genre) = meta.get_string(META_GENRE) {
        println!("    Genre: {genre}");
    }
    if let Some(stream_title) = meta.get_string(META_STREAM_TITLE) {
        println!("    Stream: {stream_title}");
    }
    if let Some(url) = meta.get_string(META_STREAM_URL) {
        println!("    URL: {url}");
    }
    if let Some(rate) = meta.get_u32(META_SAMPLE_RATE) {
        println!("    Sample Rate: {rate}Hz");
    }
    if let Some(ch) = meta.get_u16(META_CHANNELS) {
        println!("    Channels: {ch}");
    }
    if let Some(codec) = meta.get_string(META_CODEC_INFO) {
        println!("    Codec: {codec}");
    }
    if let Some(bitrate) = meta.get_u32(META_BITRATE) {
        println!("    Bitrate: {bitrate} bps");
    }
    for entry in meta.entries() {
        if entry.key >= META_CUSTOM_BASE {
            if let Ok(val) = std::str::from_utf8(&entry.value) {
                println!("    Custom[0x{:02X}]: {val}", entry.key);
            } else {
                println!(
                    "    Custom[0x{:02X}]: {} bytes",
                    entry.key,
                    entry.value.len()
                );
            }
        }
    }
}

fn rand_ssrc() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let dur = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    (dur.as_nanos() & 0xFFFF_FFFF) as u32
}
