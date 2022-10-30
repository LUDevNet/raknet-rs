use std::mem;
use std::ops::ControlFlow;
use std::time::Duration;

use crate::message::ConnectionRequest;
use crate::message::Parse;
use crate::util::MsgNumGenerator;
use crate::AckList;
use crate::BitStreamRead;
use crate::BitStreamWrite;
use crate::InternalPacket;
use crate::MessageNumberType;
use crate::PacketReliability;
use crate::Queue;
use crate::RakNetTime;
use crate::RakPeerConfig;
use crate::RemoteSystemConnectMode;
use crate::SystemAddress;
use crate::SystemIndex;
use crate::ID;
use bstr::BStr;
use num_traits::FromPrimitive;
use tokio::net::UdpSocket;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::warn;

use super::handler::PacketHandler;

pub struct RemoteSystem {
    pub(super) addr: SystemAddress,
    pub(super) connect_mode: RemoteSystemConnectMode,
    pub(super) remote_system_time: RakNetTime,
    pub(super) msg_num_gen: MsgNumGenerator,
    pub(super) acks: AckList,
    pub(super) queue: Queue,
}

impl RemoteSystem {
    pub(super) fn new(addr: SystemAddress) -> Self {
        Self {
            addr,
            connect_mode: RemoteSystemConnectMode::HandlingConnectionRequest,
            remote_system_time: 0,
            msg_num_gen: MsgNumGenerator::new(),

            acks: AckList::new(),
            queue: Queue::default(),
        }
    }

    pub fn system_address(&self) -> SystemAddress {
        self.addr
    }

    pub fn send(&mut self, bs: BitStreamWrite, reliability: PacketReliability) {
        self.queue.push(bs, reliability);
    }

    pub(super) fn on_packet<'a, H: PacketHandler>(
        &mut self,
        bytes: &'a [u8],
        local: SystemAddress,
        time: Duration,
        cfg: &RakPeerConfig,
        handler: &mut H,
    ) {
        let mut bit_stream = BitStreamRead::new(bytes);

        let acknowledgements = bit_stream.read_bool().expect("A");
        if acknowledgements {
            let _time: RakNetTime = bit_stream.read().unwrap();
            let _acks = AckList::deserialize(&mut bit_stream).unwrap();
            info!("time: {}, recv acks: {:?}", _time, _acks);
        }
        let has_time = bit_stream
            .read_bool() // NOTE: BUG in RakNet. If we reach EOF here, just ignore it.
            .unwrap_or(false);

        self.remote_system_time = match has_time {
            true => bit_stream.read().expect("C"),
            false => return,
        };
        debug!("time: {}", self.remote_system_time);

        while bit_stream.get_number_of_unread_bits() >= mem::size_of::<MessageNumberType>() << 3 {
            let internal_packet =
                InternalPacket::parse(&mut bit_stream, self.remote_system_time).expect("D");
            self.acks.insert(internal_packet.msg_num);

            let id = match ID::of_packet(internal_packet.data()) {
                Ok(opt) => opt,
                Err(e) => {
                    error!("{}", e);
                    continue;
                }
            };
            match ID::from_u8(id) {
                Some(ID::ConnectionRequest) => {
                    let req = ConnectionRequest {
                        password: BStr::new(&internal_packet.data[1..]),
                    };
                    info!("{:?}", req);

                    if req.password != cfg.incoming_password {
                        warn!("Password mismatch, disconnecting");

                        // This one we only send once since we don't care if it arrives.
                        let c = ID::InvalidPassword;
                        let mut bs = BitStreamWrite::with_capacity(8);
                        bs.write(c);
                        self.send(bs, PacketReliability::Reliable); // system priority
                        self.connect_mode = RemoteSystemConnectMode::DisconnectAsapSilently;
                    } else {
                        let mut payload = BitStreamWrite::with_capacity(15 << 3);
                        payload.write(ID::ConnectionRequestAccepted as u8);
                        payload.write_bytes(&self.addr.ip().octets(), 4);
                        payload.write(self.addr.port());
                        payload.write::<SystemIndex>(1);
                        payload.write_bytes(&local.ip().octets(), 4);
                        payload.write(local.port());
                        self.send(payload, PacketReliability::Reliable);
                    }
                }
                Some(ID::NewIncomingConnection) => {
                    let mut in_bit_stream = BitStreamRead::with_size(
                        internal_packet.data,
                        internal_packet.data_bit_size,
                    );
                    in_bit_stream.ignore_bits(8).unwrap();
                    match crate::message::NewIncomingConnection::from_bit_stream(&mut in_bit_stream)
                    {
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
                    match crate::message::InternalPing::from_bit_stream(&mut in_bit_stream) {
                        Ok(data) => {
                            info!("{:?}", data);
                            let mut out_bit_stream = BitStreamWrite::new();
                            out_bit_stream.write(ID::ConnectedPong);
                            out_bit_stream.write(data.send_ping_time);
                            out_bit_stream.write(time.as_millis() as RakNetTime);
                            self.send(out_bit_stream, PacketReliability::Reliable);
                        }
                        Err(e) => error!("{}", e),
                    }
                }
                Some(id) => warn!("TODO: {:?}: {} bits", id, internal_packet.data_bit_size),
                None => {
                    let bytes = internal_packet.data;
                    match handler.on_user_packet(bytes, self) {
                        ControlFlow::Continue(_) => {
                            warn!("Unhandled user packet [{}]: {:?}", id, &bytes[1..])
                        }
                        ControlFlow::Break(_) => {}
                    }
                }
            }
        }
    }

    pub(super) async fn update(&mut self, socket: &UdpSocket) -> Result<(), tokio::io::Error> {
        if self.acks.is_empty() && self.queue.is_empty() {
            return Ok(());
        }

        // FIXME: time?
        let time = 0;
        let mut output = BitStreamWrite::new();
        self.queue
            .generate_datagram(&mut output, &mut self.msg_num_gen, time, &mut self.acks);

        let buf = output.data();
        debug!("{:?}", buf);
        socket.send_to(buf, self.addr).await?;
        Ok(())
    }
}
