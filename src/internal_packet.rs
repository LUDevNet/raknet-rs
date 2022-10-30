use crate::{
    bits_to_bytes, BitStreamRead, BitStreamWrite, OrderingChannel, PacketReliability, RakNetTime,
    Result,
};

#[derive(Debug, Clone)]
pub struct Ordering {
    pub index: OrderingIndexType,
    pub channel: OrderingChannel,
}

#[derive(Debug, Clone)]
pub struct InternalPacket<'a> {
    pub time: RakNetTime,
    pub msg_num: MessageNumberType,
    pub reliability: PacketReliability,
    pub ordering: Option<Ordering>,
    pub is_split_packet: bool,
    pub data_bit_size: usize,
    pub data: &'a [u8],
}

pub type MessageNumberType = u32;
pub type OrderingIndexType = MessageNumberType;

impl<'a> InternalPacket<'a> {
    pub fn data(&self) -> &'a [u8] {
        self.data
    }

    pub fn write(&self, output: &mut BitStreamWrite) -> usize {
        let start = output.num_bits();
        output.write(self.msg_num);
        output.write_bits(self.reliability);
        if let Some(ordering) = &self.ordering {
            output.write_bits(ordering.channel);
            output.write(ordering.index);
        } else {
            assert!(!matches!(
                self.reliability,
                PacketReliability::ReliableOrdered
                    | PacketReliability::ReliableSequenced
                    | PacketReliability::UnreliableSequenced
            ));
        }
        output.write_bool(self.is_split_packet);
        if self.is_split_packet {
            unimplemented!()
        }
        output.write_compressed(self.data_bit_size as u16);
        output.write_aligned_bytes(self.data);
        output.num_bits() - start
    }

    pub fn parse(
        bit_stream: &mut BitStreamRead<'a>,
        time: RakNetTime,
    ) -> Result<InternalPacket<'a>> {
        let msg_num: MessageNumberType = bit_stream.read()?;
        let reliability: PacketReliability = bit_stream.read_bits()?;
        let ordering = if matches!(
            reliability,
            PacketReliability::ReliableOrdered
                | PacketReliability::ReliableSequenced
                | PacketReliability::UnreliableSequenced
        ) {
            // ordering channel encoded in 5 bits (from 0 to 31)
            Some(Ordering {
                channel: bit_stream.read_bits()?,
                index: bit_stream.read()?,
            })
        } else {
            None
        };
        let is_split_packet = bit_stream.read_bool()?;
        if is_split_packet {
            todo!()
        }
        let data_bit_size = bit_stream.read_compressed::<u16>()? as usize;

        let data = bit_stream.read_aligned_bytes(bits_to_bytes!(data_bit_size))?;
        Ok(InternalPacket {
            time,
            msg_num,
            ordering,
            reliability,
            is_split_packet,
            data,
            data_bit_size,
        })
    }
}
