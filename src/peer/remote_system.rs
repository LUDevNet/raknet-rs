pub enum ConnectMode {
    NoAction,
    DisconnectAsap,
    DisconnectAsapSilently,
    DisconnectOnNoAck,
    RequestedConnection,
    HandlingConnectionRequest,
    UnverifiedSender,
    SetEncryptionOnMultiple16BytePacket,
    Connected,
}
