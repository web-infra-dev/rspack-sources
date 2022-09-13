use std::{
  borrow::Cow,
  hash::{Hash, Hasher},
};

use crate::{
  helpers::{get_map, stream_chunks_of_source_map, StreamChunks},
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

#[derive(Debug, Clone)]
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

  fn map(&self, options: &MapOptions) -> Option<SourceMap> {
    get_map(self, options)
  }
}

impl Hash for SourceMapSource {
  fn hash<H: Hasher>(&self, state: &mut H) {
    "SourceMapSource".hash(state);
    self.buffer().hash(state);
    self.source_map.hash(state);
    self.original_source.hash(state);
    self.inner_source_map.hash(state);
    self.remove_original_source.hash(state);
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
    if let Some(inner_source_map) = &self.inner_source_map {
      todo!()
    } else {
      stream_chunks_of_source_map(
        &self.value,
        &self.source_map,
        on_chunk,
        on_source,
        on_name,
        options,
      )
    }
  }
}

#[cfg(test)]
mod tests {
  use crate::{BoxSource, ConcatSource, OriginalSource, RawSource};

  use super::*;

  #[test]
  fn map_correctly() {
    let inner_source_code = "Hello World\nis a test string\n";
    let inner_source = ConcatSource::new([
      Box::new(OriginalSource::new(inner_source_code, "hello-world.txt"))
        as BoxSource,
      Box::new(OriginalSource::new("Translate: ", "header.txt")),
      Box::new(RawSource::from("Other text")),
    ]);
  }

  #[test]
  fn should_handle_null_sources_and_sources_content() {
    let a = SourceMapSource::new(WithoutOriginalOptions {
      value: "hello world\n",
      name: "hello.txt",
      source_map: SourceMap::new(None, "AAAA", [""], [""], []),
    });
    let b = SourceMapSource::new(WithoutOriginalOptions {
      value: "hello world\n",
      name: "hello.txt",
      source_map: SourceMap::new(None, "AAAA", [], [], []),
    });
    let a = SourceMapSource::new(WithoutOriginalOptions {
      value: "hello world\n",
      name: "hello.txt",
      source_map: SourceMap::new(
        None,
        "AAAA",
        ["hello-source.txt"],
        ["hello world\n"],
        [],
      ),
    });
  }

  #[test]
  fn should_handle_es6_promise_correctly() {
    let code = include_str!(concat!(
      env!("CARGO_MANIFEST_DIR"),
      "/tests/es6-promise.js"
    ));
    let map = SourceMap::from_json(include_str!(concat!(
      env!("CARGO_MANIFEST_DIR"),
      "/tests/es6-promise.map"
    )))
    .unwrap();
    let inner = SourceMapSource::new(WithoutOriginalOptions {
      value: code,
      name: "es6-promise.js",
      source_map: map,
    });
    let source = ConcatSource::new([inner.clone(), inner]);
    assert_eq!(source.source(), format!("{code}{code}"));
  }

  #[test]
  fn should_not_emit_zero_sizes_mappings_when_ending_with_empty_mapping() {
    let a = SourceMapSource::new(WithoutOriginalOptions {
      value: "hello\n",
      name: "a",
      source_map: SourceMap::new(None, "AAAA;AACA", ["hello1"], [], []),
    });
    let b = SourceMapSource::new(WithoutOriginalOptions {
      value: "hi",
      name: "b",
      source_map: SourceMap::new(None, "AAAA,EAAE", ["hello2"], [], []),
    });
    let b2 = SourceMapSource::new(WithoutOriginalOptions {
      value: "hi",
      name: "b",
      source_map: SourceMap::new(None, "AAAA,EAAE", ["hello3"], [], []),
    });
    let c = SourceMapSource::new(WithoutOriginalOptions {
      value: "",
      name: "c",
      source_map: SourceMap::new(None, "AAAA", ["hello4"], [], []),
    });
    let source = ConcatSource::new([
      a.clone(),
      a.clone(),
      b.clone(),
      b.clone(),
      b2.clone(),
      b.clone(),
      c.clone(),
      c.clone(),
      b2.clone(),
      a.clone(),
      b2.clone(),
      c.clone(),
      a.clone(),
      b.clone(),
    ]);
    let map = source.map(&MapOptions::default()).unwrap();
    assert_eq!(
      map.mappings(),
      "AAAA;AAAA;ACAA,ICAA,EDAA,ECAA,EFAA;AEAA,EFAA;ACAA",
    );
  }
}
