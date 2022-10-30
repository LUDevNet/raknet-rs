use raknet::{
    message::Parse, util::MsgNumGenerator, AckList, BitStreamRead, BitStreamWrite, InternalPacket,
    MessageNumberType, PacketReliability, Queue, RakNetTime, SystemAddress, SystemIndex, ID,
};
use std::{
    mem,
    net::{Ipv4Addr, SocketAddr},
    time::{Duration, Instant},
};
use tokio::{io, net::UdpSocket};
use tracing::{debug, error, info, warn};

struct Connection {
    addr: SystemAddress,
    remote_system_time: RakNetTime,
    msg_num_gen: MsgNumGenerator,

    acks: AckList,
    queue: Queue,
}

impl Connection {
    fn send(&mut self, bs: BitStreamWrite, reliability: PacketReliability) {
        self.queue.push(bs, reliability);
    }

    fn on_packet(
        &mut self,
        bytes: &[u8],
        local: SystemAddress,
        time: Duration,
    ) -> Option<BitStreamWrite> {
        let mut bit_stream = BitStreamRead::new(bytes);

        let acknowledgements = bit_stream.read_bool().expect("A");
        if acknowledgements {
            let _time: RakNetTime = bit_stream.read().unwrap();
            let _acks = AckList::deserialize(&mut bit_stream).unwrap();
            debug!("time: {}, acks: {:?}", _time, _acks);
        }
        let has_time = bit_stream
            .read_bool() // NOTE: BUG in RakNet. If we reach EOF here, just ignore it.
            .unwrap_or(false);

        self.remote_system_time = match has_time {
            true => bit_stream.read().expect("C"),
            false => return None,
        };
        debug!("time: {}", self.remote_system_time);

        while bit_stream.get_number_of_unread_bits() >= mem::size_of::<MessageNumberType>() << 3 {
            let internal_packet =
                InternalPacket::parse(&mut bit_stream, self.remote_system_time).expect("D");
            self.acks.insert(internal_packet.msg_num);

            let id = ID::of_packet(internal_packet.data());
            match id {
                Some(ID::ConnectionRequest) => {
                    // TODO: any remaining bytes are the password

                    info!("Connection Request: {} bits", internal_packet.data_bit_size);
                    let mut payload = BitStreamWrite::new();
                    payload.write(ID::ConnectionRequestAccepted as u8);
                    payload.write_bytes(&self.addr.ip().octets(), 4);
                    payload.write(self.addr.port());
                    payload.write::<SystemIndex>(1);
                    payload.write_bytes(&local.ip().octets(), 4);
                    payload.write(local.port());
                    self.send(payload, PacketReliability::Reliable);
                }
                Some(ID::NewIncomingConnection) => {
                    let mut in_bit_stream = BitStreamRead::with_size(
                        internal_packet.data,
                        internal_packet.data_bit_size,
                    );
                    in_bit_stream.ignore_bits(8).unwrap();
                    match raknet::message::NewIncomingConnection::from_bit_stream(
                        &mut in_bit_stream,
                    ) {
                        Ok(data) => info!("{:?}", data),
                        Err(e) => error!("{}", e),
                    }
                }
                Some(ID::DisconnectionNotification) => {
                    info!("DisconnectionNotification");
                }
                Some(ID::InternalPing) => {
                    let mut in_bit_stream = BitStreamRead::with_size(
                        internal_packet.data,
                        internal_packet.data_bit_size,
                    );
                    in_bit_stream.ignore_bits(8).unwrap();
                    match raknet::message::InternalPing::from_bit_stream(&mut in_bit_stream) {
                        Ok(data) => {
                            info!("{:?}", data);
                            let mut out_bit_stream = BitStreamWrite::new();
                            out_bit_stream.write(ID::ConnectedPong);
                            out_bit_stream.write(data.send_ping_time);
                            out_bit_stream.write(time.as_millis() as RakNetTime);
                            self.send(out_bit_stream, PacketReliability::Reliable);
                        }
                        Err(e) => eprintln!("{}", e),
                    }
                }
                Some(id) => warn!("TODO: {:?}: {} bits", id, internal_packet.data_bit_size),
                None => error!("Unknown packet data: {:?}", &internal_packet.data),
            }
        }
        None
    }

    async fn update(&mut self, socket: &UdpSocket) -> Result<(), tokio::io::Error> {
        if self.acks.is_empty() && self.queue.is_empty() {
            return Ok(());
        }

        // FIXME: time?
        let time = 0;
        let mut output = BitStreamWrite::new();
        self.queue
            .generate_datagram(&mut output, &mut self.msg_num_gen, time, &mut self.acks);
        socket.send_to(output.data(), self.addr).await?;
        Ok(())
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), io::Error> {
    tracing_subscriber::fmt::init();
    let local = SystemAddress::new(Ipv4Addr::LOCALHOST, 2000);
    let socket = UdpSocket::bind(local).await?;
    let start = Instant::now();
    info!("Server started on {:?}", local);

    let mut buf = vec![0; 2048];
    let mut connections: Vec<Connection> = Vec::new();

    loop {
        let (length, remote) = socket.recv_from(&mut buf).await?;
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
            connection.on_packet(bytes, local, Instant::now().duration_since(start));
            connection.update(&socket).await?;
        } else {
            match ID::of_packet(bytes) {
                Some(id) => {
                    debug!("raw: {:?}", id);
                    match id {
                        ID::OpenConnectionRequest => {
                            let reply = if true {
                                connections.push(Connection {
                                    addr: origin,
                                    remote_system_time: 0,
                                    msg_num_gen: MsgNumGenerator::new(),

                                    acks: AckList::new(),
                                    queue: Queue::default(),
                                });
                                ID::OpenConnectionReply
                            } else {
                                ID::NoFreeIncomingConnections
                            };
                            socket.send_to(&[reply as u8, 0], origin).await?;
                        }
                        _ => debug!("bytes: {:?}", &bytes[1..]),
                    }
                }
                None => error!("Missing or invalid first byte from {}: {:?}", origin, bytes),
            }
        }
    }
}
