/// These enumerations are used to describe when packets are delivered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum PacketPriority {
    /// \internal Used by RakNet to send above-high priority messages.
    System,

    /// High priority messages are send before medium priority messages.
    High,

    /// Medium priority messages are send before low priority messages.
    Medium,

    /// Low priority messages are only sent when no other messages are waiting.
    Low,

    #[doc(hidden)]
    P4,
    #[doc(hidden)]
    P5,
    #[doc(hidden)]
    P6,
    #[doc(hidden)]
    P7,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(transparent)]
pub struct OrderingChannel(u8);

/// These enumerations are used to describe how packets are delivered.
/// \note  Note to self: I write this with 3 bits in the stream.  If I add more remember to change that
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum PacketReliability {
    /// Same as regular UDP, except that it will also discard duplicate datagrams.  RakNet adds (6 to 17) + 21 bits of overhead, 16 of which is used to detect duplicate packets and 6 to 17 of which is used for message length.
    Unreliable,

    /// Regular UDP with a sequence counter.  Out of order messages will be discarded.  This adds an additional 13 bits on top what is used for UNRELIABLE.
    UnreliableSequenced,

    /// The message is sent reliably, but not necessarily in any order.  Same overhead as UNRELIABLE.
    Reliable,

    /// This message is reliable and will arrive in the order you sent it.  Messages will be delayed while waiting for out of order messages.  Same overhead as UNRELIABLE_SEQUENCED.
    ReliableOrdered,

    /// This message is reliable and will arrive in the sequence you sent it.  Out or order messages will be dropped.  Same overhead as UNRELIABLE_SEQUENCED.
    ReliableSequenced,

    #[doc(hidden)]
    R5,
    #[doc(hidden)]
    R6,
    #[doc(hidden)]
    R7,
}
