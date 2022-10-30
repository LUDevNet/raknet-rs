mod identifiers;
use std::net::Ipv4Addr;

pub use identifiers::ID;

use crate::{BitStreamRead, RakNetTime, Result, SystemAddress};

pub trait Parse: Sized {
    fn from_bit_stream(bs: &mut BitStreamRead) -> Result<Self>;
}

#[derive(Debug)]
pub struct NewIncomingConnection {
    pub local: SystemAddress,
    pub remote: SystemAddress,
}

impl Parse for NewIncomingConnection {
    fn from_bit_stream(bs: &mut BitStreamRead) -> Result<Self> {
        Ok(Self {
            local: SystemAddress::new(Ipv4Addr::from(bs.read::<u32>()?), bs.read()?),
            remote: SystemAddress::new(Ipv4Addr::from(bs.read::<u32>()?), bs.read()?),
        })
    }
}

#[derive(Debug)]
pub struct InternalPing {
    pub send_ping_time: RakNetTime,
}

impl Parse for InternalPing {
    fn from_bit_stream(bs: &mut BitStreamRead) -> Result<Self> {
        Ok(Self {
            send_ping_time: bs.read()?,
        })
    }
}
