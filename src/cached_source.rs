use std::{
  borrow::Cow,
  hash::{BuildHasherDefault, Hash},
  sync::{Arc, OnceLock},
};

use dashmap::DashMap;
use rustc_hash::FxHasher;

use crate::{
  helpers::{
    stream_and_get_source_and_map, stream_chunks_of_raw_source,
    stream_chunks_of_source_map, StreamChunks,
  },
  MapOptions, Source, SourceMap,
};

/// It tries to reused cached results from other methods to avoid calculations,
/// usually used after modify is finished.
///
/// - [webpack-sources docs](https://github.com/webpack/webpack-sources/#cachedsource).
///
/// ```
/// use rspack_sources::{
///   BoxSource, CachedSource, ConcatSource, MapOptions, OriginalSource,
///   RawSource, Source, SourceExt, SourceMap,
/// };
///
/// let mut concat = ConcatSource::new([
///   RawSource::from("Hello World\n".to_string()).boxed(),
///   OriginalSource::new(
///     "console.log('test');\nconsole.log('test2');\n",
///     "console.js",
///   )
///   .boxed(),
/// ]);
/// concat.add(OriginalSource::new("Hello2\n", "hello.md"));
///
/// let cached = CachedSource::new(concat);
///
/// assert_eq!(
///   cached.source(),
///   "Hello World\nconsole.log('test');\nconsole.log('test2');\nHello2\n"
/// );
/// // second time will be fast.
/// assert_eq!(
///   cached.source(),
///   "Hello World\nconsole.log('test');\nconsole.log('test2');\nHello2\n"
/// );
/// ```
pub struct CachedSource<T> {
  inner: Arc<T>,
  cached_buffer: OnceLock<Arc<Vec<u8>>>,
  cached_source: OnceLock<Arc<str>>,
  cached_maps:
    Arc<DashMap<MapOptions, Option<SourceMap>, BuildHasherDefault<FxHasher>>>,
}

impl<T> CachedSource<T> {
  /// Create a [CachedSource] with the original [Source].
  pub fn new(inner: T) -> Self {
    Self {
      inner: Arc::new(inner),
      cached_buffer: Default::default(),
      cached_source: Default::default(),
      cached_maps: Default::default(),
    }
  }

  /// Get the original [Source].
  pub fn original(&self) -> &T {
    &self.inner
  }
}

impl<T: Source + Hash + PartialEq + Eq + 'static> Source for CachedSource<T> {
  fn source(&self) -> Cow<str> {
    let cached = self
      .cached_source
      .get_or_init(|| self.inner.source().into());
    Cow::Borrowed(cached)
  }

  fn buffer(&self) -> Cow<[u8]> {
    let cached = self.cached_buffer.get_or_init(|| {
      let source = self.cached_source.get();
      match source {
        Some(source) => source.as_bytes().to_vec().into(),
        None => self.inner.buffer().to_vec().into(),
      }
    });
    Cow::Borrowed(cached)
  }

  fn size(&self) -> usize {
    self.inner.size()
  }

  fn map(&self, options: &MapOptions) -> Option<SourceMap> {
    if let Some(map) = self.cached_maps.get(options) {
      map.clone()
    } else {
      let map = self.inner.map(options);
      self.cached_maps.insert(options.clone(), map.clone());
      map
    }
  }

  fn to_writer(&self, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
    self.inner.to_writer(writer)
  }
}

impl<T: Source + Hash + PartialEq + Eq + 'static> StreamChunks
  for CachedSource<T>
{
  fn stream_chunks(
    &self,
    options: &MapOptions,
    on_chunk: crate::helpers::OnChunk,
    on_source: crate::helpers::OnSource,
    on_name: crate::helpers::OnName,
  ) -> crate::helpers::GeneratedInfo {
    if self.cached_maps.contains_key(options) {
      let source = self.source();
      if let Some(map) = &self.map(options) {
        return stream_chunks_of_source_map(
          &source, map, on_chunk, on_source, on_name, options,
        );
      } else {
        return stream_chunks_of_raw_source(
          &source, options, on_chunk, on_source, on_name,
        );
      }
    }
    let (generated_info, map) = stream_and_get_source_and_map(
      self.original(),
      options,
      on_chunk,
      on_source,
      on_name,
    );
    self.cached_maps.insert(options.clone(), map);
    generated_info
  }
}

impl<T: Source> Clone for CachedSource<T> {
  fn clone(&self) -> Self {
    Self {
      inner: self.inner.clone(),
      cached_buffer: self.cached_buffer.clone(),
      cached_source: self.cached_source.clone(),
      cached_maps: self.cached_maps.clone(),
    }
  }
}

impl<T: Hash> Hash for CachedSource<T> {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.inner.hash(state);
  }
}

impl<T: PartialEq> PartialEq for CachedSource<T> {
  fn eq(&self, other: &Self) -> bool {
    self.inner == other.inner
  }
}

impl<T: Eq> Eq for CachedSource<T> {}

impl<T: std::fmt::Debug> std::fmt::Debug for CachedSource<T> {
  fn fmt(
    &self,
    f: &mut std::fmt::Formatter<'_>,
  ) -> Result<(), std::fmt::Error> {
    f.debug_struct("CachedSource")
      .field("inner", self.inner.as_ref())
      .field("cached_buffer", &self.cached_buffer.get().is_some())
      .field("cached_source", &self.cached_source.get().is_some())
      .field("cached_maps", &(!self.cached_maps.is_empty()))
      .finish()
  }
}

#[cfg(test)]
mod tests {
  use crate::{
    ConcatSource, RawSource, SourceExt, SourceMapSource, WithoutOriginalOptions,
  };

  use super::*;

  #[test]
  fn line_number_should_not_add_one() {
    let source = ConcatSource::new([
      CachedSource::new(RawSource::from("\n")).boxed(),
      SourceMapSource::new(WithoutOriginalOptions {
        value: "\nconsole.log(1);\n".to_string(),
        name: "index.js".to_string(),
        source_map: SourceMap::new(
          None,
          ";AACA".to_string(),
          vec!["index.js".into()],
          vec!["// DELETE IT\nconsole.log(1)".into()],
          vec![],
        ),
      })
      .boxed(),
    ]);
    let map = source.map(&Default::default()).unwrap();
    assert_eq!(map.mappings(), ";;AACA");
  }
}
