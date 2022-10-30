use std::fmt;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Error {
    BitStreamEos,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BitStreamEos => write!(f, "BitStream read past EOS"),
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
