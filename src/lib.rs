mod bit_stream;
pub mod ds;
mod error;
mod internal_packet;
pub mod message;
mod packet_priority;
mod peer;
mod reliability_layer;
mod types;

pub use bit_stream::{BitSize, BitStreamRead, BitStreamWrite};
pub use error::{Error, Result};
pub use internal_packet::{InternalPacket, MessageNumberType, Ordering, OrderingIndexType};
pub use message::ID;
pub use packet_priority::{OrderingChannel, PacketPriority, PacketReliability};
pub use reliability_layer::AckList;
pub use types::{RakNetTime, SystemAddress, SystemIndex};

#[macro_export]
macro_rules! bits_to_bytes {
    ($x:expr) => {
        (($x) + 7) >> 3
    };
}

#[macro_export]
macro_rules! bytes_to_bits {
    ($x:expr) => {
        (x) << 3
    };
}
