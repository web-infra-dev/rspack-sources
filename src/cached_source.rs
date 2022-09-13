use std::{borrow::Cow, collections::HashMap};

use parking_lot::Mutex;
use smol_str::SmolStr;

use crate::{
  helpers::{
    stream_chunks_of_raw_source, stream_chunks_of_source_map, StreamChunks,
  },
  MapOptions, Source, SourceMap,
};

#[derive(Debug)]
pub struct CachedSource<T> {
  inner: T,
  cached_buffer: Mutex<Option<Vec<u8>>>,
  cached_source: Mutex<Option<SmolStr>>,
  cached_maps: Mutex<HashMap<MapOptions, Option<SourceMap>>>,
}

impl<T> CachedSource<T> {
  pub fn new(inner: T) -> Self {
    Self {
      inner,
      cached_buffer: Mutex::new(None),
      cached_source: Mutex::new(None),
      cached_maps: Mutex::new(HashMap::new()),
    }
  }

  pub fn inner(&self) -> &T {
    &self.inner
  }
}

impl<T: Source> Source for CachedSource<T> {
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

impl<T: Source> StreamChunks for CachedSource<T> {
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
