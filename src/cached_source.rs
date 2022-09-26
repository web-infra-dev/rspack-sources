use std::{borrow::Cow, collections::HashMap, hash::Hash};

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
  cached_buffer: Mutex<Option<Vec<u8>>>,
  cached_source: Mutex<Option<SmolStr>>,
  cached_maps: Mutex<HashMap<MapOptions, Option<SourceMap>>>,
}

impl<T> CachedSource<T> {
  /// Create a [CachedSource] with the original [Source].
  pub fn new(inner: T) -> Self {
    Self {
      inner,
      cached_buffer: Mutex::new(None),
      cached_source: Mutex::new(None),
      cached_maps: Mutex::new(HashMap::new()),
    }
  }

  /// Get the original [Source].
  pub fn original(&self) -> &T {
    &self.inner
  }
}

impl<T: Source> Clone for CachedSource<T> {
  fn clone(&self) -> Self {
    Self {
      inner: dyn_clone::clone(&self.inner),
      cached_buffer: Mutex::new(self.cached_buffer.lock().clone()),
      cached_source: Mutex::new(self.cached_source.lock().clone()),
      cached_maps: Mutex::new(self.cached_maps.lock().clone()),
    }
  }
}

impl<T: Source + Hash> Source for CachedSource<T> {
  fn source(&self) -> Cow<str> {
    let mut cached_source = self.cached_source.lock();
    if let Some(cached_source) = &*cached_source {
      Cow::Owned(cached_source.to_string())
    } else {
      let source = self.inner.source().to_string();
      *cached_source = Some(SmolStr::new(source.clone()));
      Cow::Owned(source)
    }
  }

  fn buffer(&self) -> Cow<[u8]> {
    let mut cached_buffer = self.cached_buffer.lock();
    if let Some(cached_buffer) = &*cached_buffer {
      Cow::Owned(cached_buffer.to_owned())
    } else {
      let buffer = self.inner.buffer().to_vec();
      *cached_buffer = Some(buffer.clone());
      Cow::Owned(buffer)
    }
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

impl<T: Hash> Hash for CachedSource<T> {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.inner.hash(state);
  }
}

impl<T: Source + Hash> StreamChunks for CachedSource<T> {
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
