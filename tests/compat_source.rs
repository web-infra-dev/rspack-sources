#![allow(missing_docs)]
use std::borrow::Cow;
use std::hash::Hash;

use rspack_sources::stream_chunks::{
  stream_chunks_default, GeneratedInfo, OnChunk, OnName, OnSource, StreamChunks,
};
use rspack_sources::{
  ConcatSource, MapOptions, RawSource, Rope, Source, SourceExt, SourceMap,
};

#[derive(Debug, Eq)]
struct CompatSource(&'static str, Option<SourceMap>);

impl Source for CompatSource {
  fn source(&self) -> Cow<str> {
    Cow::Borrowed(self.0)
  }

  fn rope(&self) -> Rope<'_> {
    Rope::from(self.0)
  }

  fn buffer(&self) -> Cow<[u8]> {
    Cow::Borrowed(self.0.as_bytes())
  }

  fn size(&self) -> usize {
    42
  }

  fn map(&self, _options: &MapOptions) -> Option<SourceMap> {
    self.1.clone()
  }

  fn to_writer(&self, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
    writer.write_all(self.0.as_bytes())
  }
}

impl StreamChunks for CompatSource {
  fn stream_chunks<'a>(
    &'a self,
    options: &MapOptions,
    on_chunk: OnChunk<'_, 'a>,
    on_source: OnSource<'_, 'a>,
    on_name: OnName<'_, 'a>,
  ) -> GeneratedInfo {
    stream_chunks_default(
      self.0,
      self.1.as_ref(),
      options,
      on_chunk,
      on_source,
      on_name,
    )
  }
}

impl Hash for CompatSource {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    "__CompatSource".hash(state);
    self.0.hash(state);
  }
}

impl PartialEq for CompatSource {
  fn eq(&self, other: &Self) -> bool {
    self.0 == other.0
  }
}

impl Clone for CompatSource {
  fn clone(&self) -> Self {
    Self(self.0, self.1.clone())
  }
}

#[test]
fn should_work_with_custom_compat_source() {
  const CONTENT: &str = "Line1\n\nLine3\n";

  let source = CompatSource(CONTENT, None);
  assert_eq!(source.source(), CONTENT);
  assert_eq!(source.size(), 42);
  assert_eq!(source.buffer(), CONTENT.as_bytes());
  assert_eq!(source.map(&MapOptions::default()), None);
}

#[test]
fn should_generate_correct_source_map() {
  let source_map = SourceMap::from_json(
    r#"{
      "version": 3,
      "sources": ["compat.js"],
      "sourcesContent": ["Line1\n\nLine3\n"],
      "mappings": "AAAA;AACA;AACA",
      "names": []
    }"#,
  )
  .unwrap();

  let result = ConcatSource::new([
    RawSource::from("Line0\n").boxed(),
    CompatSource("Line1\nLine2\nLine3\n", Some(source_map)).boxed(),
  ]);

  let source = result.source();
  let map = result.map(&MapOptions::default()).unwrap();

  let expected_source = "Line0\nLine1\nLine2\nLine3\n";
  let expected_source_map = SourceMap::from_json(
    r#"{
      "version": 3,
      "sources": ["compat.js"],
      "sourcesContent": ["Line1\n\nLine3\n"],
      "mappings": ";AAAA;AACA;AACA",
      "names": []
    }"#,
  )
  .unwrap();

  assert_eq!(source, expected_source);
  assert_eq!(map, expected_source_map)
}
