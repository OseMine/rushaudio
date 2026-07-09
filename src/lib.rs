pub mod protocol;
pub mod transport;
pub mod session;
pub mod audio;
pub mod utils;

pub use protocol::*;
pub use transport::*;
pub use session::*;
pub use audio::*;
pub use utils::*;

pub mod prelude {
    pub use crate::audio::{AudioCodecManager, JitterBuffer, JitterStats};
    pub use crate::protocol::{
        constants::*, types::*, Packet, PacketHeader, PacketType, StreamConfig, StreamStats,
    };
    pub use crate::session::{Handshake, HandshakeRole, HandshakeState, SessionManager};
    pub use crate::transport::{Connection, ConnectionPool, UdpTransport};
    pub use crate::utils::FecEncoder;
}

/// Version of the RushAudio protocol implementation.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Default port for RushAudio streaming.
pub const DEFAULT_PORT: u16 = 4210;

/// Creates a new server bound to the given address.
/// `bind_addr` like `"0.0.0.0:4210"`.
pub fn create_server(bind_addr: &str) -> std::io::Result<UdpTransport> {
    UdpTransport::bind(bind_addr)
}

/// Creates a new client connected to a remote server.
pub fn create_client() -> std::io::Result<UdpTransport> {
    UdpTransport::bind("0.0.0.0:0")
}
