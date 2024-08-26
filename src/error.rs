use std::{error, fmt, result};

/// An alias for [std::result::Result<T, rspack_sources::Error>].
pub type Result<T> = result::Result<T, Error>;

/// Error for this crate.
#[derive(Debug)]
pub enum Error {
  /// a VLQ string was malformed and data was left over
  VlqLeftover,
  /// a VLQ string was empty and no values could be decoded.
  VlqNoValues,
  /// Unexpectedly hit EOF.
  VlqUnexpectedEof,
  /// Overflow in Vlq handling
  VlqOverflow,
  /// a JSON parsing related failure
  BadJson(simd_json::Error),
}

impl fmt::Display for Error {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Error::VlqLeftover => write!(f, "leftover cur/shift in vlq decode"),
      Error::VlqNoValues => write!(f, "vlq decode did not produce any values"),
      Error::VlqUnexpectedEof => write!(f, "vlq decode unexpected eof"),
      Error::VlqOverflow => write!(f, "vlq decode caused an overflow"),
      Error::BadJson(err) => write!(f, "bad json: {err}"),
    }
  }
}

impl error::Error for Error {}

impl From<simd_json::Error> for Error {
  fn from(err: simd_json::Error) -> Error {
    Error::BadJson(err)
  }
}
