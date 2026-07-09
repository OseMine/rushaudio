use std::net::{SocketAddr, ToSocketAddrs};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use rushaudio::prelude::*;

static PHASE: AtomicU32 = AtomicU32::new(0);

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let server_addr = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| format!("127.0.0.1:{}", rushaudio::DEFAULT_PORT));
    let track_title = args
        .get(2)
        .map(|s| s.as_str())
        .unwrap_or("RushAudio Test Tone");
    let artist = args
        .get(3)
        .map(|s| s.as_str())
        .unwrap_or("RushAudio Client");

    let server_socket: SocketAddr = server_addr
        .to_socket_addrs()
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "bad address"))?
        .next()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no address"))?;

    println!("[Client] Connecting to RushAudio server at {server_socket}");
    println!("[Client] Track: {track_title} by {artist}");

    let mut transport = rushaudio::create_client()?;
    let local_ssrc = rand_ssrc();
    let config = StreamConfig::default();
    let mut handshake = Handshake::new(HandshakeRole::Initiator, local_ssrc, config.clone());
    let mut session_store: Option<SessionManager> = None;
    let mut connected = false;
    let mut seq: u32 = 0;
    let mut audio_seq: u32 = 0;
    let mut last_keepalive = Instant::now();
    let mut last_metadata = Instant::now();
    let stream_active = true;

    let _stream = start_audio_playback();

    {
        let hs_pkt = handshake.build_request(seq, 0);
        transport.send_packet(&hs_pkt, server_socket)?;
        println!("[Client] Sent handshake request (SSRC={local_ssrc})");
        handshake.start(server_socket);
        seq += 1;
    }

    let mut last_audio = Instant::now();

    loop {
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

                            send_metadata(
                                &mut transport,
                                &mut seq,
                                server_socket,
                                local_ssrc,
                                track_title,
                                artist,
                            );
                        } else {
                            eprintln!("[Client] Handshake rejected");
                            break;
                        }
                    }
                }

                PacketType::Metadata => {
                    if let Ok(meta) = Metadata::from_packet(&packet) {
                        println!("[Client] Server sent metadata:");
                        if let Some(name) = meta.get_string(META_STREAM_TITLE) {
                            println!("    Stream: {name}");
                        }
                        for entry in meta.entries() {
                            if entry.key >= META_CUSTOM_BASE {
                                if let Ok(val) = std::str::from_utf8(&entry.value) {
                                    println!("    Custom[0x{:02X}]: {val}", entry.key);
                                }
                            }
                        }
                    }
                }

                PacketType::KeepAlive => {
                    if let Some(ref mut mgr) = session_store {
                        mgr.handle_keepalive(&server_socket);
                    }
                }

                _ => {
                    println!(
                        "[Client] Received {:?} from server",
                        packet.header.packet_type
                    );
                }
            }
        }

        if connected && stream_active && last_audio.elapsed() > Duration::from_millis(20) {
            let timestamp = last_audio.elapsed().as_millis() as u32;
            let pcm_frame = generate_test_tone(48000, 2, 20, audio_seq);

            let audio_payload = Packet::encode_audio_payload(2, 48000, 0x02, &pcm_frame);
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

        // Re-send metadata every 5 seconds to simulate dynamic updates
        if connected && last_metadata.elapsed() > Duration::from_secs(5) {
            let elapsed = last_metadata.elapsed().as_secs();
            let updated_title = format!("{track_title} [{elapsed}s]");
            send_metadata(
                &mut transport,
                &mut seq,
                server_socket,
                local_ssrc,
                &updated_title,
                artist,
            );
            last_metadata = Instant::now();
        }

        if connected && last_keepalive.elapsed() > Duration::from_millis(2000) {
            if let Some(ref mut mgr) = session_store {
                if let Some(pkt) = mgr.send_keepalive(&server_socket) {
                    let _ = transport.send_packet(&pkt, server_socket);
                }
            }
            last_keepalive = Instant::now();
        }

        if !connected && handshake.timed_out() {
            eprintln!("[Client] Handshake timed out");
            break;
        }

        std::thread::sleep(Duration::from_millis(1));
    }

    if connected {
        let stop_pkt = Packet::new(PacketType::StreamControl, seq, 0, vec![CONTROL_STREAM_STOP]);
        let _ = transport.send_packet(&stop_pkt, server_socket);
        println!("[Client] Sent stream stop");
    }

    println!("[Client] Shutting down");
    Ok(())
}

fn send_metadata(
    transport: &mut UdpTransport,
    seq: &mut u32,
    dest: SocketAddr,
    ssrc: u32,
    title: &str,
    artist: &str,
) {
    let meta = MetadataBuilder::new()
        .ssrc(ssrc)
        .track_title(title)
        .artist(artist)
        .stream_title("Live RushAudio Stream")
        .album("Demo Album")
        .genre("Electronic")
        .sample_rate(48000)
        .channels(2)
        .codec_info("RawPCMI16")
        .bitrate(1536000)
        .stream_url("http://localhost:4210")
        .custom(0x80, b"demo-client-v1".to_vec())
        .custom(0x81, format!("session {}", ssrc).into_bytes())
        .build();

    let meta_pkt = meta.to_packet(*seq, 0);
    if let Err(e) = transport.send_packet(&meta_pkt, dest) {
        eprintln!("[Client] Failed to send metadata: {e}");
    } else {
        println!("[Client] Sent metadata: \"{title}\" by {artist}");
    }
    *seq += 1;
}

fn start_audio_playback() -> cpal::Stream {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .expect("no output device available");
    println!(
        "[Client] Using audio output: {}",
        device.name().unwrap_or_default()
    );

    let supported = device
        .default_output_config()
        .expect("no default output config");
    println!(
        "[Client] Audio config: {}Hz, {}ch, {:?}",
        supported.sample_rate().0,
        supported.channels(),
        supported.sample_format()
    );

    let sample_rate = supported.sample_rate().0;
    let channels = supported.channels() as u32;
    let stream_config: cpal::StreamConfig = supported.into();

    let stream = device
        .build_output_stream(
            &stream_config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                let freq = 440.0_f32;
                for frame in data.chunks_mut(channels as usize) {
                    let phase = PHASE.fetch_add(1, Ordering::Relaxed);
                    let t = phase as f32 / sample_rate as f32;
                    let value = (t * freq * 2.0 * std::f32::consts::PI).sin() * 0.3;
                    for sample in frame.iter_mut() {
                        *sample = value;
                    }
                }
            },
            |err| eprintln!("[Client] Audio error: {err}"),
            None,
        )
        .expect("failed to build output stream");

    stream.play().expect("failed to start audio stream");
    println!("[Client] Playing 440Hz tone on speakers (press Ctrl+C to stop)");
    stream
}

fn generate_test_tone(sample_rate: u32, channels: u16, duration_ms: u64, phase: u32) -> Vec<u8> {
    let samples_per_ch = (sample_rate as u64 * duration_ms) / 1000;
    let total_samples = samples_per_ch * channels as u64;
    let mut buf = Vec::with_capacity(total_samples as usize * 2);

    let freq = 440.0;
    let amplitude: f32 = 0.5;

    for i in 0..total_samples {
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
    let dur = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    (dur.as_nanos() & 0xFFFF_FFFF) as u32
}
