use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::time::Duration;

use crate::protocol::{Packet, MAX_PACKET_SIZE};

pub struct UdpTransport {
    socket: UdpSocket,
    read_buf: Vec<u8>,
}

impl UdpTransport {
    pub fn bind(addr: &str) -> io::Result<Self> {
        let parsed: SocketAddr = addr.parse().map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        let socket = UdpSocket::bind(parsed)?;
        socket.set_nonblocking(true)?;
        Ok(Self {
            socket,
            read_buf: vec![0u8; MAX_PACKET_SIZE],
        })
    }

    pub fn send_packet(&self, packet: &Packet, dest: SocketAddr) -> io::Result<usize> {
        let encoded = packet.encode();
        self.socket.send_to(&encoded, dest)
    }

    pub fn send_raw(&self, data: &[u8], dest: SocketAddr) -> io::Result<usize> {
        self.socket.send_to(data, dest)
    }

    pub fn recv_packet(&mut self) -> io::Result<Option<(Packet, SocketAddr)>> {
        loop {
            match self.socket.recv_from(&mut self.read_buf) {
                Ok((n, src)) => {
                    let data = &self.read_buf[..n];
                    match Packet::decode(data) {
                        Ok(pkt) => return Ok(Some((pkt, src))),
                        Err(_) => continue,
                    }
                }
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                    return Ok(None);
                }
                Err(e) => return Err(e),
            }
        }
    }

    pub fn set_recv_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.socket.set_read_timeout(timeout)
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.socket.local_addr()
    }

    pub fn socket(&self) -> &UdpSocket {
        &self.socket
    }

    pub fn flush_read(&mut self) -> usize {
        let mut count = 0;
        while let Ok(Some((_, _))) = self.recv_packet() {
            count += 1;
        }
        count
    }
}
