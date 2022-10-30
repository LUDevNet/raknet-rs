use std::ops::ControlFlow;

pub trait PacketHandler {
    fn on_user_packet(&mut self, bytes: &[u8]) -> ControlFlow<()>;
}
