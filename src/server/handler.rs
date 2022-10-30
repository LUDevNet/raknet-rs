use std::ops::ControlFlow;

use super::connection::RemoteSystem;

pub trait PacketHandler {
    fn on_user_packet(&mut self, bytes: &[u8], conn: &mut RemoteSystem) -> ControlFlow<()>;
}
