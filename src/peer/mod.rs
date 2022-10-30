use bstr::BString;

mod remote_system;
pub use remote_system::ConnectMode as RemoteSystemConnectMode;

/// Configuration for RakPeers
pub struct RakPeerConfig {
    pub max_incoming_connections: usize,
    pub incoming_password: BString,
}
