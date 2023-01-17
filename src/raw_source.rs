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

/// Represents source code without source map, it will not create source map for the source code.
///
/// - [webpack-sources docs](https://github.com/webpack/webpack-sources/#rawsource).
///
/// ```
/// use rspack_sources::{MapOptions, RawSource, Source};
///
/// let code = "some source code";
/// let s = RawSource::from(code.to_string());
/// assert_eq!(s.source(), code);
/// assert_eq!(s.map(&MapOptions::default()), None);
/// assert_eq!(s.size(), 16);
/// ```
#[derive(Debug, Clone, Eq)]
pub enum RawSource {
  /// Represent buffer.
  Buffer(Vec<u8>),
  /// Represent string.
  Source(String),
}

impl RawSource {
  /// Whether the [RawSource] represent a buffer.
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

  fn to_writer(&self, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
    writer.write_all(match self {
      RawSource::Buffer(i) => i,
      RawSource::Source(i) => i.as_bytes(),
    })
  }
}

impl Hash for RawSource {
  fn hash<H: Hasher>(&self, state: &mut H) {
    "RawSource".hash(state);
    self.buffer().hash(state);
  }
}

impl PartialEq for RawSource {
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (Self::Buffer(l0), Self::Buffer(r0)) => l0 == r0,
      (Self::Source(l0), Self::Source(r0)) => l0 == r0,
      _ => false,
    }
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
