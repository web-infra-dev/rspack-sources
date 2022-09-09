use std::{error, fmt, result};

pub type Result<T> = result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
  /// a VLQ string was malformed and data was left over
  VlqLeftover,
  /// a VLQ string was empty and no values could be decoded.
  VlqNoValues,
  /// Overflow in Vlq handling
  VlqOverflow,
  /// a mapping segment had an unsupported size
  BadSegmentSize(u32),
  /// a reference to a non existing source was encountered
  BadSourceReference(u32),
  /// a reference to a non existing name was encountered
  BadNameReference(u32),
}

impl fmt::Display for Error {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match *self {
      Error::VlqLeftover => write!(f, "leftover cur/shift in vlq decode"),
      Error::VlqNoValues => write!(f, "vlq decode did not produce any values"),
      Error::VlqOverflow => write!(f, "vlq decode caused an overflow"),
      Error::BadSegmentSize(size) => {
        write!(f, "got {} segments, expected 4 or 5", size)
      }
      Error::BadSourceReference(id) => {
        write!(f, "bad reference to source #{}", id)
      }
      Error::BadNameReference(id) => write!(f, "bad reference to name #{}", id),
    }
  }
}

impl error::Error for Error {}
