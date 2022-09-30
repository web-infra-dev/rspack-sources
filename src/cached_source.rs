use std::{borrow::Cow, hash::Hash};

use hashbrown::HashMap;
use once_cell::sync::OnceCell;
use parking_lot::Mutex;
use smol_str::SmolStr;

use crate::{
  helpers::{
    stream_chunks_of_raw_source, stream_chunks_of_source_map, StreamChunks,
  },
  MapOptions, Source, SourceMap,
};

/// It tries to reused cached results from other methods to avoid calculations,
/// usally used after modify is finished.
///
/// - [webpack-sources docs](https://github.com/webpack/webpack-sources/#cachedsource).
///
/// ```
/// use rspack_sources::{
///   BoxSource, CachedSource, ConcatSource, MapOptions, OriginalSource,
///   RawSource, Source, SourceMap,
/// };
///
/// let mut concat = ConcatSource::new([
///   Box::new(RawSource::from("Hello World\n".to_string())) as BoxSource,
///   Box::new(OriginalSource::new(
///     "console.log('test');\nconsole.log('test2');\n",
///     "console.js",
///   )),
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
#[derive(Debug)]
pub struct CachedSource<T> {
  inner: T,
  cached_buffer: OnceCell<Vec<u8>>,
  cached_source: OnceCell<SmolStr>,
  cached_maps: Mutex<HashMap<MapOptions, Option<SourceMap>>>,
}

impl<T> CachedSource<T> {
  /// Create a [CachedSource] with the original [Source].
  pub fn new(inner: T) -> Self {
    Self {
      inner,
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
      .get_or_init(|| SmolStr::from(self.inner.source()));
    Cow::Owned(cached.to_string())
  }

  fn buffer(&self) -> Cow<[u8]> {
    let cached = self
      .cached_buffer
      .get_or_init(|| self.inner.buffer().to_vec());
    Cow::Owned(cached.clone())
  }

  fn size(&self) -> usize {
    self.inner.size()
  }

  fn map(&self, options: &MapOptions) -> Option<SourceMap> {
    let mut cached_maps = self.cached_maps.lock();
    if let Some(map) = cached_maps.get(options) {
      map.clone()
    } else {
      let map = self.inner.map(options);
      cached_maps.insert(options.to_owned(), map.clone());
      map
    }
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
    let source = self.source();
    if let Some(map) = &self.map(options) {
      stream_chunks_of_source_map(
        &source, map, on_chunk, on_source, on_name, options,
      )
    } else {
      stream_chunks_of_raw_source(
        &source, options, on_chunk, on_source, on_name,
      )
    }
  }
}

impl<T: Source> Clone for CachedSource<T> {
  fn clone(&self) -> Self {
    Self {
      inner: dyn_clone::clone(&self.inner),
      cached_buffer: self.cached_buffer.clone(),
      cached_source: self.cached_source.clone(),
      cached_maps: Mutex::new(self.cached_maps.lock().clone()),
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
      && self.cached_buffer == other.cached_buffer
      && self.cached_source == other.cached_source
      && *self.cached_maps.lock() == *other.cached_maps.lock()
  }
}

impl<T: Eq> Eq for CachedSource<T> {}
