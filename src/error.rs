#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Tls(rustls::Error),
    //Quic(s2n_quic::provider::tls::default::error::Error),
    Quic(quiche::Error),
}
pub type Result<T> = std::result::Result<T, Error>;

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err)
    }
}
impl From<rustls::Error> for Error {
    fn from(err: rustls::Error) -> Self {
        Error::Tls(err)
    }
}
// impl From<s2n_quic::provider::tls::default::error::Error> for Error {
//     fn from(err: s2n_quic::provider::tls::default::error::Error) -> Self {
//         Error::Quic(err)
//     }
// }
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(err) => write!(f, "IO error: {}", err),
            Error::Tls(err) => write!(f, "TLS error: {}", err),
            Error::Quic(err) => write!(f, "QUIC error: {}", err),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(err) => Some(err),
            Error::Tls(err) => Some(err),
            Error::Quic(err) => Some(err),
        }
    }
}
