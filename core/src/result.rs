use std::fmt;
use std::string;

#[derive(Debug, Clone)]
pub enum RspackSourcesError {
  UTF8Error,
}

#[derive(Debug)]
pub struct Error {
  pub error_type: RspackSourcesError,
  pub reason: Option<String>,
}

impl Error {
  pub fn new(error_type: RspackSourcesError) -> Self {
    Self {
      error_type,
      reason: None,
    }
  }

  pub fn new_with_reason(error_type: RspackSourcesError, reason: &str) -> Self {
    Self {
      error_type,
      reason: Some(String::from(reason)),
    }
  }
}

impl fmt::Display for Error {
  #[inline]
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    if let Some(ref reason) = self.reason {
      write!(f, "{:?}, {}", self.error_type, reason)
    } else {
      write!(f, "{:?}", self.error_type)
    }
  }
}

impl From<string::FromUtf8Error> for Error {
  #[inline]
  fn from(_: string::FromUtf8Error) -> Self {
    Error::new(RspackSourcesError::UTF8Error)
  }
}
