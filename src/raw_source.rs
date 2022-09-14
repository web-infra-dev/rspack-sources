use std::{
  borrow::Cow,
  hash::{Hash, Hasher},
};

use crate::{
  helpers::{
    get_generated_source_info, stream_chunks_of_raw_source, OnChunk, OnName,
    OnSource, StreamChunks,
  },
  MapOptions, Source, SourceMap,
};

#[derive(Debug, Clone)]
pub enum RawSource {
  Buffer(Vec<u8>),
  Source(String),
}

impl RawSource {
  pub fn is_buffer(&self) -> bool {
    matches!(self, Self::Buffer(_))
  }
}

impl From<String> for RawSource {
  fn from(s: String) -> Self {
    Self::Source(s)
  }
}

impl From<Vec<u8>> for RawSource {
  fn from(s: Vec<u8>) -> Self {
    Self::Buffer(s)
  }
}

impl From<&str> for RawSource {
  fn from(s: &str) -> Self {
    Self::Source(s.to_owned())
  }
}

impl From<&[u8]> for RawSource {
  fn from(s: &[u8]) -> Self {
    Self::Buffer(s.to_owned())
  }
}

impl Source for RawSource {
  fn source(&self) -> Cow<str> {
    match self {
      RawSource::Buffer(i) => String::from_utf8_lossy(i),
      RawSource::Source(i) => Cow::Borrowed(i),
    }
  }

  fn buffer(&self) -> Cow<[u8]> {
    match self {
      RawSource::Buffer(i) => Cow::Borrowed(i),
      RawSource::Source(i) => Cow::Borrowed(i.as_bytes()),
    }
  }

  fn size(&self) -> usize {
    match self {
      RawSource::Buffer(i) => i.len(),
      RawSource::Source(i) => i.len(),
    }
  }

  fn map(&self, _: &MapOptions) -> Option<SourceMap> {
    None
  }
}

impl Hash for RawSource {
  fn hash<H: Hasher>(&self, state: &mut H) {
    "RawSource".hash(state);
    self.buffer().hash(state);
  }
}

impl StreamChunks for RawSource {
  fn stream_chunks(
    &self,
    options: &MapOptions,
    on_chunk: OnChunk,
    on_source: OnSource,
    on_name: OnName,
  ) -> crate::helpers::GeneratedInfo {
    if options.final_source {
      get_generated_source_info(&self.source())
    } else {
      stream_chunks_of_raw_source(
        &self.source(),
        options,
        on_chunk,
        on_source,
        on_name,
      )
    }
  }
}
