use std::collections::VecDeque;

use tracing::debug;

use crate::{
    util::MsgNumGenerator, AckList, BitStreamWrite, InternalPacket, PacketReliability, RakNetTime,
};

#[derive(Default)]
pub struct Queue {
    inner: VecDeque<(BitStreamWrite, PacketReliability)>,
}

impl Queue {
    pub fn push(&mut self, bs: BitStreamWrite, reliability: PacketReliability) {
        self.inner.push_back((bs, reliability));
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn generate_datagram(
        &mut self,
        output: &mut BitStreamWrite,
        msg_num_gen: &mut MsgNumGenerator,
        time: RakNetTime,
        acks: &mut AckList,
    ) {
        debug!(
            "Generating Datagram for acks {:?} and {} packets",
            acks,
            self.inner.len()
        );
        let mut wrote_data = false;

        if acks.is_empty() {
            output.write_0();
        } else {
            output.write_1(); // has acks
            output.write(0u32); // time
            acks.serialize(output);
            acks.clear(); // FIXME: Clear only written
        }

        if let Some((payload, reliability)) = self.inner.pop_front() {
            // FIXME: limit
            let msg_num = msg_num_gen.next();
            let data_bit_size = payload.num_bits();
            debug!(
                "Writing {:?} packet #{}, bitlen {}: {:?}",
                reliability,
                msg_num,
                data_bit_size,
                payload.data()
            );
            let internal_packet = InternalPacket {
                time,
                msg_num,
                reliability,
                ordering: None,
                is_split_packet: false,
                data_bit_size,
                data: payload.data(),
            };

            if !wrote_data {
                output.write_1();
                output.write(time);
                wrote_data = true;
            }
            let _bits_written = internal_packet.write(output);
        }

        debug!("queue.len: {}", self.inner.len());

        if !wrote_data {
            output.write_0();
        }
    }
}
