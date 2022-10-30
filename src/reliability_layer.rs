use crate::internal_packet::MessageNumberType;

pub type AckList = crate::ds::RangeList<1, MessageNumberType>;
