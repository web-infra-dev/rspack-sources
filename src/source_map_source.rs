use std::borrow::Cow;

use crate::{
  helpers::{
    create_mapping_serializer, get_map, MappingSerializer, StreamChunks,
  },
  MapOptions, Source, SourceMap,
};

#[derive(Debug, Clone)]
pub struct SourceMapSourceOptions<V, N> {
  pub value: V,
  pub name: N,
  pub source_map: SourceMap,
  pub original_source: Option<String>,
  pub inner_source_map: Option<SourceMap>,
  pub remove_original_source: bool,
}

#[derive(Debug, Clone)]
pub struct WithoutOriginalOptions<V, N> {
  pub value: V,
  pub name: N,
  pub source_map: SourceMap,
}

impl<V, N> From<WithoutOriginalOptions<V, N>> for SourceMapSourceOptions<V, N> {
  fn from(options: WithoutOriginalOptions<V, N>) -> Self {
    Self {
      value: options.value,
      name: options.name,
      source_map: options.source_map,
      original_source: None,
      inner_source_map: None,
      remove_original_source: false,
    }
  }
}

pub struct SourceMapSource {
  value: String,
  name: String,
  source_map: SourceMap,
  original_source: Option<String>,
  inner_source_map: Option<SourceMap>,
  remove_original_source: bool,
}

impl SourceMapSource {
  pub fn new<V, N, O>(options: O) -> Self
  where
    V: Into<String>,
    N: Into<String>,
    O: Into<SourceMapSourceOptions<V, N>>,
  {
    let options = options.into();
    Self {
      value: options.value.into(),
      name: options.name.into(),
      source_map: options.source_map,
      original_source: options.original_source,
      inner_source_map: options.inner_source_map,
      remove_original_source: options.remove_original_source,
    }
  }
}

impl Source for SourceMapSource {
  fn source(&self) -> Cow<str> {
    Cow::Borrowed(&self.value)
  }

  fn buffer(&self) -> Cow<[u8]> {
    Cow::Borrowed(self.value.as_bytes())
  }

  fn size(&self) -> usize {
    self.value.len()
  }

  fn map(&self, options: MapOptions) -> Option<SourceMap> {
    get_map(self, options)
  }
}

impl StreamChunks for SourceMapSource {
  fn stream_chunks(
    &self,
    options: &MapOptions,
    on_chunk: crate::helpers::OnChunk,
    on_source: crate::helpers::OnSource,
    on_name: crate::helpers::OnName,
  ) -> crate::helpers::GeneratedInfo {
    if let Some(inner_source_map) = self.inner_source_map {
      todo!()
    } else {
    }
  }
}
