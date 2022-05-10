use crate::{Error, GenMapOption, Source};
use sourcemap::SourceMap;

pub struct RawSource {
  source_code: String,
}

impl RawSource {
  pub fn new(source_code: String) -> Self {
    Self { source_code }
  }

  pub fn from_slice(source_code: &[u8]) -> Result<Self, Error> {
    Ok(Self {
      source_code: String::from_utf8(source_code.to_vec())?,
    })
  }
}

impl Source for RawSource {
  fn map(&mut self, _option: &GenMapOption) -> Option<SourceMap> {
    None
  }

  fn source(&mut self) -> String {
    self.source_code.clone()
  }
}
