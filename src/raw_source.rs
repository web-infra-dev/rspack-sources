use std::{
  borrow::Cow,
  hash::{Hash, Hasher},
  sync::OnceLock,
};

use ouroboros::self_referencing;

use crate::{
  helpers::{
    get_generated_source_info, stream_chunks_of_raw_source, OnChunk, OnName,
    OnSource, StreamChunks,
  },
  MapOptions, Rope, Source, SourceMap,
};

/// A string variant of RawSource.
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
    let indent = f.width().unwrap_or(0);
    let indent_str = format!("{:indent$}", "", indent = indent);
    write!(
      f,
      "{indent_str}RawStringSource::from_static({:?}).boxed()",
      self.0.as_ref()
    )
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
      get_generated_source_info(&*self.0)
    } else {
      stream_chunks_of_raw_source(
        &*self.0, options, on_chunk, on_source, on_name,
      )
    }
  }
}

/// A buffer variant of RawSource.
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
#[self_referencing]
pub struct RawBufferSource {
  value: Vec<u8>,
  #[borrows(value)]
  #[not_covariant]
  value_as_string: OnceLock<Cow<'this, str>>,
}

impl RawBufferSource {
  fn get_or_init_value_as_string(&self) -> &str {
    self.with(|fields| {
      fields
        .value_as_string
        .get_or_init(|| String::from_utf8_lossy(fields.value))
    })
  }
}

impl Clone for RawBufferSource {
  fn clone(&self) -> Self {
    RawBufferSourceBuilder {
      value: self.borrow_value().clone(),
      value_as_string_builder: |_: &Vec<u8>| Default::default(),
    }
    .build()
  }
}

impl PartialEq for RawBufferSource {
  fn eq(&self, other: &Self) -> bool {
    self.borrow_value() == other.borrow_value()
  }
}

impl Eq for RawBufferSource {}

impl From<Vec<u8>> for RawBufferSource {
  fn from(value: Vec<u8>) -> Self {
    RawBufferSourceBuilder {
      value,
      value_as_string_builder: |_: &Vec<u8>| Default::default(),
    }
    .build()
  }
}

impl From<&[u8]> for RawBufferSource {
  fn from(value: &[u8]) -> Self {
    RawBufferSourceBuilder {
      value: value.to_vec(),
      value_as_string_builder: |_: &Vec<u8>| Default::default(),
    }
    .build()
  }
}

impl Source for RawBufferSource {
  fn source(&self) -> Cow<str> {
    Cow::Borrowed(self.get_or_init_value_as_string())
  }

  fn rope(&self) -> Rope<'_> {
    Rope::from(self.get_or_init_value_as_string())
  }

  fn buffer(&self) -> Cow<[u8]> {
    Cow::Borrowed(self.borrow_value())
  }

  fn size(&self) -> usize {
    self.borrow_value().len()
  }

  fn map(&self, _: &MapOptions) -> Option<SourceMap> {
    None
  }

  fn to_writer(&self, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
    writer.write_all(self.borrow_value())
  }
}

impl std::fmt::Debug for RawBufferSource {
  fn fmt(
    &self,
    f: &mut std::fmt::Formatter<'_>,
  ) -> Result<(), std::fmt::Error> {
    let indent = f.width().unwrap_or(0);
    let indent_str = format!("{:indent$}", "", indent = indent);
    write!(
      f,
      "{indent_str}RawBufferSource::from({:?}).boxed()",
      self.borrow_value()
    )
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
      get_generated_source_info(&*self.source())
    } else {
      stream_chunks_of_raw_source(
        self.get_or_init_value_as_string(),
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
    let source1 = RawStringSource::from("hello\n\n");
    let source1 = ReplaceSource::new(source1);
    let source2 = OriginalSource::new("world".to_string(), "world.txt");
    let concat = ConcatSource::new([source1.boxed(), source2.boxed()]);
    let map = concat.map(&MapOptions::new(false)).unwrap();
    assert_eq!(map.mappings(), ";;AAAA",);
  }
}
