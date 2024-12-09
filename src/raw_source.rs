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
  MapOptions, Rope, Source, SourceMap,
};

#[derive(Clone, PartialEq, Eq)]
enum RawValue {
  Buffer(Vec<u8>),
  String(Cow<'static, str>),
}

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
static_assertions::assert_eq_size!(RawValue, [u8; 32]);

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

impl RawSource {
  /// Create a new [RawSource] from a static &str.
  ///
  /// ```
  /// use rspack_sources::{RawSource, Source};
  ///
  /// let code = "some source code";
  /// let s = RawSource::from_static(code);
  /// assert_eq!(s.source(), code);
  /// ```
  pub fn from_static(s: &'static str) -> Self {
    Self {
      value: RawValue::String(Cow::Borrowed(s)),
      value_as_string: Default::default(),
    }
  }
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
      value: RawValue::String(value.into()),
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
      value: RawValue::String(value.to_string().into()),
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

  fn rope(&self) -> Rope<'_> {
    match &self.value {
      RawValue::Buffer(v) => Rope::from(
        self
          .value_as_string
          .get_or_init(|| String::from_utf8_lossy(v).to_string()),
      ),
      RawValue::String(s) => Rope::from(s),
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

impl StreamChunks for RawSource {
  fn stream_chunks<'a>(
    &'a self,
    options: &MapOptions,
    on_chunk: OnChunk<'_, 'a>,
    on_source: OnSource<'_, 'a>,
    on_name: OnName<'_, 'a>,
  ) -> crate::helpers::GeneratedInfo {
    if options.final_source {
      match &self.value {
        RawValue::Buffer(buffer) => {
          let source = self
            .value_as_string
            .get_or_init(|| String::from_utf8_lossy(buffer).to_string());
          get_generated_source_info(&Rope::from_str(source))
        }
        RawValue::String(source) => {
          get_generated_source_info(&Rope::from_str(source))
        }
      }
    } else {
      match &self.value {
        RawValue::Buffer(buffer) => {
          let source = self
            .value_as_string
            .get_or_init(|| String::from_utf8_lossy(buffer).to_string());
          stream_chunks_of_raw_source(
            Rope::from_str(source),
            options,
            on_chunk,
            on_source,
            on_name,
          )
        }
        RawValue::String(source) => stream_chunks_of_raw_source(
          Rope::from_str(source),
          options,
          on_chunk,
          on_source,
          on_name,
        ),
      }
    }
  }
}

/// A string variant of [RawSource].
///
/// - [webpack-sources docs](https://github.com/webpack/webpack-sources/#rawsource).
///
/// ```
/// use rspack_sources::{MapOptions, RawStringSource, Source};
///
/// let code = "some source code";
/// let s = RawStringSource::from(code.to_string());
/// assert_eq!(s.source(), code);
/// assert_eq!(s.map(&MapOptions::default()), None);
/// assert_eq!(s.size(), 16);
/// ```
#[derive(Clone, PartialEq, Eq)]
pub struct RawStringSource(Cow<'static, str>);

#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
static_assertions::assert_eq_size!(RawStringSource, [u8; 24]);

impl RawStringSource {
  /// Create a new [RawStringSource] from a static &str.
  ///
  /// ```
  /// use rspack_sources::{RawStringSource, Source};
  ///
  /// let code = "some source code";
  /// let s = RawStringSource::from_static(code);
  /// assert_eq!(s.source(), code);
  /// ```
  pub fn from_static(s: &'static str) -> Self {
    Self(Cow::Borrowed(s))
  }
}

impl From<String> for RawStringSource {
  fn from(value: String) -> Self {
    Self(Cow::Owned(value))
  }
}

impl From<&str> for RawStringSource {
  fn from(value: &str) -> Self {
    Self(Cow::Owned(value.to_owned()))
  }
}

impl Source for RawStringSource {
  fn source(&self) -> Cow<str> {
    Cow::Borrowed(&self.0)
  }

  fn rope(&self) -> Rope<'_> {
    Rope::from(&self.0)
  }

  fn buffer(&self) -> Cow<[u8]> {
    Cow::Borrowed(self.0.as_bytes())
  }

  fn size(&self) -> usize {
    self.0.len()
  }

  fn map(&self, _: &MapOptions) -> Option<SourceMap> {
    None
  }

  fn to_writer(&self, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
    writer.write_all(self.0.as_bytes())
  }
}

impl std::fmt::Debug for RawStringSource {
  fn fmt(
    &self,
    f: &mut std::fmt::Formatter<'_>,
  ) -> Result<(), std::fmt::Error> {
    let mut d = f.debug_tuple("RawStringSource");
    d.field(&self.0.chars().take(50).collect::<String>());
    d.finish()
  }
}

impl Hash for RawStringSource {
  fn hash<H: Hasher>(&self, state: &mut H) {
    "RawStringSource".hash(state);
    self.buffer().hash(state);
  }
}

impl StreamChunks for RawStringSource {
  fn stream_chunks<'a>(
    &'a self,
    options: &MapOptions,
    on_chunk: OnChunk<'_, 'a>,
    on_source: OnSource<'_, 'a>,
    on_name: OnName<'_, 'a>,
  ) -> crate::helpers::GeneratedInfo {
    if options.final_source {
      get_generated_source_info(&Rope::from_str(&self.source()))
    } else {
      stream_chunks_of_raw_source(
        Rope::from_str(&self.0),
        options,
        on_chunk,
        on_source,
        on_name,
      )
    }
  }
}

/// A buffer variant of [RawSource].
///
/// - [webpack-sources docs](https://github.com/webpack/webpack-sources/#rawsource).
///
/// ```
/// use rspack_sources::{MapOptions, RawBufferSource, Source};
///
/// let code = "some source code".as_bytes();
/// let s = RawBufferSource::from(code);
/// assert_eq!(s.buffer(), code);
/// assert_eq!(s.map(&MapOptions::default()), None);
/// assert_eq!(s.size(), 16);
/// ```
#[derive(Clone, PartialEq, Eq)]
pub struct RawBufferSource {
  value: Vec<u8>,
  value_as_string: OnceLock<String>,
}

impl From<Vec<u8>> for RawBufferSource {
  fn from(value: Vec<u8>) -> Self {
    Self {
      value,
      value_as_string: Default::default(),
    }
  }
}

impl From<&[u8]> for RawBufferSource {
  fn from(value: &[u8]) -> Self {
    Self {
      value: value.to_vec(),
      value_as_string: Default::default(),
    }
  }
}

impl Source for RawBufferSource {
  fn source(&self) -> Cow<str> {
    Cow::Borrowed(
      self
        .value_as_string
        .get_or_init(|| String::from_utf8_lossy(&self.value).to_string()),
    )
  }

  fn rope(&self) -> Rope<'_> {
    Rope::from(
      self
        .value_as_string
        .get_or_init(|| String::from_utf8_lossy(&self.value).to_string()),
    )
  }

  fn buffer(&self) -> Cow<[u8]> {
    Cow::Borrowed(&self.value)
  }

  fn size(&self) -> usize {
    self.value.len()
  }

  fn map(&self, _: &MapOptions) -> Option<SourceMap> {
    None
  }

  fn to_writer(&self, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
    writer.write_all(&self.value)
  }
}

impl std::fmt::Debug for RawBufferSource {
  fn fmt(
    &self,
    f: &mut std::fmt::Formatter<'_>,
  ) -> Result<(), std::fmt::Error> {
    let mut d = f.debug_tuple("RawBufferSource");
    d.field(&self.value.iter().take(50).copied().collect::<Vec<u8>>());
    d.finish()
  }
}

impl Hash for RawBufferSource {
  fn hash<H: Hasher>(&self, state: &mut H) {
    "RawBufferSource".hash(state);
    self.buffer().hash(state);
  }
}

impl StreamChunks for RawBufferSource {
  fn stream_chunks<'a>(
    &'a self,
    options: &MapOptions,
    on_chunk: OnChunk<'_, 'a>,
    on_source: OnSource<'_, 'a>,
    on_name: OnName<'_, 'a>,
  ) -> crate::helpers::GeneratedInfo {
    if options.final_source {
      get_generated_source_info(&Rope::from_str(&self.source()))
    } else {
      stream_chunks_of_raw_source(
        Rope::from_str(
          self
            .value_as_string
            .get_or_init(|| String::from_utf8_lossy(&self.value).to_string()),
        ),
        options,
        on_chunk,
        on_source,
        on_name,
      )
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
    let source1 = RawSource::from("hello\n\n".to_string());
    let source1 = ReplaceSource::new(source1);
    let source2 = OriginalSource::new("world".to_string(), "world.txt");
    let concat = ConcatSource::new([source1.boxed(), source2.boxed()]);
    let map = concat.map(&MapOptions::new(false)).unwrap();
    assert_eq!(map.mappings(), ";;AAAA",);
  }
}
