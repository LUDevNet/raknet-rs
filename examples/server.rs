use argh::FromArgs;
use bstr::BStr;
use raknet::PacketHandler;
use raknet::RakPeer;
use raknet::RemoteSystem;
use raknet::SystemAddress;
use std::net::Ipv4Addr;
use std::ops::ControlFlow;
use tokio::io;
use tracing::info;

#[derive(FromArgs)]
/// Reach new heights.
struct TestServer {
    /// the password to use
    #[argh(option, short = 'P', default = "String::new()")]
    password: String,

    /// how high to go
    #[argh(option, short = 'p', default = "2000")]
    port: u16,
}

struct BasicHandler;

impl PacketHandler for BasicHandler {
    fn on_user_packet(&mut self, bytes: &[u8], conn: &mut RemoteSystem) -> ControlFlow<()> {
        info!(
            "user packet from {}: {:?}",
            conn.system_address(),
            BStr::new(bytes)
        );
        ControlFlow::Break(())
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), io::Error> {
    tracing_subscriber::fmt::init();
    let args: TestServer = argh::from_env();

    let local = SystemAddress::new(Ipv4Addr::LOCALHOST, args.port);
    let mut server = RakPeer::new(local, BasicHandler).await?;
    server.run(args.password).await?;
    Ok(())
}
