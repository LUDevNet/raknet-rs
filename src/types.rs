use std::{net::SocketAddrV4, num::TryFromIntError, time::Duration};

use crate::bit_stream::{ReadSafe, WriteSafe};

#[repr(transparent)]
#[derive(Debug, Copy, Clone)]
pub struct RakNetTime(pub(crate) u32);

impl RakNetTime {
    pub const ZERO: Self = Self(0);
}

impl TryFrom<Duration> for RakNetTime {
    type Error = TryFromIntError;

    fn try_from(value: Duration) -> Result<Self, Self::Error> {
        Ok(Self(value.as_millis().try_into()?))
    }
}

// SAFETY: repr(transparent) and u32: ReadSafe
unsafe impl ReadSafe for RakNetTime {}
// SAFETY: repr(transparent) and u32: WriteSafe
unsafe impl WriteSafe for RakNetTime {}

pub type SystemIndex = u16;
pub type SystemAddress = SocketAddrV4;
