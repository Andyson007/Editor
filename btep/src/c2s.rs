//! mdoule for client updates sendt to the server

/// S2C or Server to Client
/// Encodes information that originates from the client and sendt to the server
pub enum C2S {
    Char(char),
    Backspace,
    Infallible(!)
}
