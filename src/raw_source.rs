use std::{
  borrow::Cow,
  hash::{Hash, Hasher},
  sync::OnceLock,
};

use crate::{
  helpers::{
    get_generated_source_info, stream_chunks_of_raw_source, OnChunk, OnName,
    OnSource, StreamChunks,
  },
  MapOptions, Source, SourceMap,
};

#[derive(Clone, PartialEq, Eq)]
enum RawValue {
  Buffer(Vec<u8>),
  String(String),
}

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
pub struct RawSource {
  value: RawValue,
  value_as_string: OnceLock<String>,
}

impl Clone for RawSource {
  fn clone(&self) -> Self {
    Self {
      value: self.value.clone(),
      value_as_string: Default::default(),
    }
  }
}

impl Eq for RawSource {}

impl RawSource {
  /// Whether the [RawSource] represent a buffer.
  pub fn is_buffer(&self) -> bool {
    matches!(self.value, RawValue::Buffer(_))
  }
}

impl From<String> for RawSource {
  fn from(value: String) -> Self {
    Self {
      value: RawValue::String(value),
      value_as_string: Default::default(),
    }
  }
}

impl From<Vec<u8>> for RawSource {
  fn from(value: Vec<u8>) -> Self {
    Self {
      value: RawValue::Buffer(value),
      value_as_string: Default::default(),
    }
  }
}

impl From<&str> for RawSource {
  fn from(value: &str) -> Self {
    Self {
      value: RawValue::String(value.to_string()),
      value_as_string: Default::default(),
    }
  }
}

impl From<&[u8]> for RawSource {
  fn from(value: &[u8]) -> Self {
    Self {
      value: RawValue::Buffer(value.to_owned()),
      value_as_string: Default::default(),
    }
  }
}

impl Source for RawSource {
  fn source(&self) -> Cow<str> {
    match &self.value {
      RawValue::String(v) => Cow::Borrowed(v),
      RawValue::Buffer(v) => Cow::Borrowed(
        self
          .value_as_string
          .get_or_init(|| String::from_utf8_lossy(v).to_string()),
      ),
    }
  }

  fn buffer(&self) -> Cow<[u8]> {
    match &self.value {
      RawValue::String(v) => Cow::Borrowed(v.as_bytes()),
      RawValue::Buffer(v) => Cow::Borrowed(v),
    }
  }

  fn size(&self) -> usize {
    match &self.value {
      RawValue::String(v) => v.len(),
      RawValue::Buffer(v) => v.len(),
    }
  }

  fn map(&self, _: &MapOptions) -> Option<SourceMap> {
    None
  }

  fn to_writer(&self, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
    writer.write_all(match &self.value {
      RawValue::String(v) => v.as_bytes(),
      RawValue::Buffer(v) => v,
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
    if std::ptr::eq(self, other) {
      return true;
    }
    match (&self.value, &other.value) {
      (RawValue::Buffer(l0), RawValue::Buffer(r0)) => l0 == r0,
      (RawValue::String(l0), RawValue::String(r0)) => l0 == r0,
      _ => false,
    }
  }
}

impl std::fmt::Debug for RawSource {
  fn fmt(
    &self,
    f: &mut std::fmt::Formatter<'_>,
  ) -> Result<(), std::fmt::Error> {
    let mut d = f.debug_struct("RawSource");
    match &self.value {
      RawValue::Buffer(buffer) => {
        d.field(
          "buffer",
          &buffer.iter().take(50).copied().collect::<Vec<u8>>(),
        );
      }
      RawValue::String(string) => {
        d.field("source", &string.chars().take(50).collect::<String>());
      }
    }
    d.finish()
  }
}

impl<'a> StreamChunks<'a> for RawSource {
  fn stream_chunks(
    &'a self,
    options: &MapOptions,
    on_chunk: OnChunk<'_, 'a>,
    on_source: OnSource<'_, 'a>,
    on_name: OnName<'_, 'a>,
  ) -> crate::helpers::GeneratedInfo {
    if options.final_source {
      get_generated_source_info(&self.source())
    } else {
      match &self.value {
        RawValue::Buffer(buffer) => {
          let source = self
            .value_as_string
            .get_or_init(|| String::from_utf8_lossy(buffer).to_string());
          stream_chunks_of_raw_source(
            source, options, on_chunk, on_source, on_name,
          )
        }
        RawValue::String(source) => stream_chunks_of_raw_source(
          source, options, on_chunk, on_source, on_name,
        ),
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use crate::{ConcatSource, OriginalSource, ReplaceSource, SourceExt};

  use super::*;

  // Fix https://github.com/web-infra-dev/rspack/issues/6793
  #[test]
  fn fix_rspack_issue_6793() {
    let source1 = RawSource::from("hello\n\n".to_string()).boxed();
    let source1 = ReplaceSource::new(source1);
    let source2 = OriginalSource::new("world".to_string(), "world.txt");
    let concat = ConcatSource::new([source1.boxed(), source2.boxed()]);
    let map = concat.map(&MapOptions::new(false)).unwrap();
    assert_eq!(map.mappings(), ";;AAAA",);
  }
}
