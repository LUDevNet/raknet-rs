mod connection;
mod handler;

use bstr::BString;
use num_traits::FromPrimitive;
use std::{
    net::{SocketAddr, SocketAddrV4},
    time::Instant,
};
use tokio::net::UdpSocket;
use tracing::{debug, error, info, warn};

use crate::{RakPeerConfig, ID};
pub use connection::RemoteSystem;
pub use handler::PacketHandler;

pub struct RakPeer<H> {
    handler: H,
    socket: UdpSocket,
    local: SocketAddrV4,
}

impl<H: PacketHandler> RakPeer<H> {
    pub async fn new(local: SocketAddrV4, handler: H) -> Result<Self, tokio::io::Error> {
        let socket = UdpSocket::bind(local).await?;
        info!("Server started on {:?}", local);
        Ok(Self {
            socket,
            local,
            handler,
        })
    }

    pub async fn run(&mut self, password: String) -> Result<(), tokio::io::Error> {
        let start = Instant::now();
        let mut buf = vec![0; 2048];
        let mut connections: Vec<RemoteSystem> = Vec::new();
        let peer_config = RakPeerConfig {
            max_incoming_connections: 10,
            incoming_password: BString::from(password),
        };

        loop {
            for conn in &mut connections {
                conn.update(&self.socket).await?;
            }

            let (length, remote) = self.socket.recv_from(&mut buf).await?;
            let origin = match remote {
                SocketAddr::V4(v4) => v4,
                SocketAddr::V6(_v6) => {
                    eprintln!("IPv6 not supported");
                    continue;
                }
            };
            let bytes = &buf[..length];

            let conn = connections.iter_mut().find(|x| x.addr == origin);

            debug!("{} bytes from {}", bytes.len(), origin);
            if let Some(connection) = conn {
                connection.on_packet(
                    bytes,
                    self.local,
                    Instant::now().duration_since(start),
                    &peer_config,
                    &mut self.handler,
                );
                connection.update(&self.socket).await?;
                if connection.queue.is_empty() && connection.pending_disconnect() {
                    if let Some(index) = connections.iter_mut().position(|x| x.addr == origin) {
                        connections.remove(index);
                    } else {
                        warn!("Failed to find connection {} to remove!", origin);
                    }
                }
            } else {
                let id_byte = match ID::of_packet(bytes) {
                    Ok(b) => b,
                    Err(e) => {
                        error!("{}", e);
                        continue;
                    }
                };
                match ID::from_u8(id_byte) {
                    Some(id) => {
                        debug!("raw: {:?}", id);
                        match id {
                            ID::OpenConnectionRequest => {
                                let reply = if true {
                                    connections.push(RemoteSystem::new(origin));
                                    ID::OpenConnectionReply
                                } else {
                                    ID::NoFreeIncomingConnections
                                };
                                self.socket.send_to(&[reply as u8, 0], origin).await?;
                            }
                            _ => debug!("bytes: {:?}", &bytes[1..]),
                        }
                    }
                    None => error!("Missing or invalid first byte from {}: {:?}", origin, bytes),
                }
            }
        }
    }
}
