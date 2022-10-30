use std::fmt;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Error {
    BitStreamEos,
    MissingID,
    MissingTimedID,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BitStreamEos => write!(f, "BitStream read past EOS"),
            Self::MissingID => write!(f, "Missing packet ID"),
            Self::MissingTimedID => write!(f, "Missing packet ID after timestamp"),
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
