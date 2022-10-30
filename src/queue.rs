use crate::{
    util::MsgNumGenerator, AckList, BitStreamWrite, InternalPacket, PacketReliability, RakNetTime,
};

#[derive(Default)]
pub struct Queue {
    inner: Vec<(BitStreamWrite, PacketReliability)>,
}

impl Queue {
    pub fn push(&mut self, bs: BitStreamWrite, reliability: PacketReliability) {
        self.inner.push((bs, reliability));
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
        let mut wrote_data = false;

        if acks.is_empty() {
            output.write_0();
        } else {
            output.write_1(); // has acks
            output.write(0u32); // time
            acks.serialize(output);
            acks.clear(); // FIXME: Clear only written
        }

        for (payload, reliability) in self.inner.drain(..) {
            // FIXME: limit
            let internal_packet = InternalPacket {
                time,
                msg_num: msg_num_gen.next(),
                reliability,
                ordering: None,
                is_split_packet: false,
                data_bit_size: payload.num_bits(),
                data: payload.data(),
            };

            if !wrote_data {
                output.write_1();
                output.write(time);
                wrote_data = true;
            }
            internal_packet.write(output);
        }

        if !wrote_data {
            output.write_0();
        }
    }
}
