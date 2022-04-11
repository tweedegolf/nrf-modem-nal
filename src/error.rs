#[derive(Debug, Clone)]
pub enum Error {
    NrfModem(nrfxlib::Error),
    NrfSys(i32),
    AddressNotFound,
    HostnameTooLong,
    HostnameNotAscii,
    SocketAlreadyOpen,
    SocketClosed,
    Fmt(core::fmt::Error),
    AtParsing(at_commands::parser::ParseError),
    NoAtResponse,
    UnexpectedAtResponse,
    InvalidConfiguration,
    NotAllowedInActiveState,
    InvalidBandConfiguration,
    /// A buffer was too small. The number indicates how big the buffer has to be (if that can be determined).
    BufferTooSmall(Option<usize>),
}

impl From<nrfxlib::Error> for Error {
    fn from(e: nrfxlib::Error) -> Self {
        Self::NrfModem(e)
    }
}
impl From<core::fmt::Error> for Error {
    fn from(e: core::fmt::Error) -> Self {
        Self::Fmt(e)
    }
}
impl From<at_commands::parser::ParseError> for Error {
    fn from(e: at_commands::parser::ParseError) -> Self {
        Self::AtParsing(e)
    }
}
