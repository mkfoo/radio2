#[derive(Debug)]
pub enum Error {
    EmptySegment,
    NoVariantStream,
    ParseError,
}

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Error::*;
        match self {
            EmptySegment => write!(f, "got empty segment from server"),
            NoVariantStream => write!(f, "no variant stream found"),
            ParseError => write!(f, "playlist parsing error"),
        }
    }
}
