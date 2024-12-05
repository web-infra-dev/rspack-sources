use std::{
  any::{Any, TypeId},
  borrow::Cow,
  convert::{TryFrom, TryInto},
  fmt,
  hash::{Hash, Hasher},
  sync::Arc,
};

use serde::{Deserialize, Serialize};

use crate::{
  helpers::{decode_mappings, StreamChunks},
  Result,
};

/// An alias for `Box<dyn Source>`.
pub type BoxSource = Arc<dyn Source>;

/// [Source] abstraction, [webpack-sources docs](https://github.com/webpack/webpack-sources/#source).
pub trait Source:
  for<'a> StreamChunks<'a> + DynHash + AsAny + DynEq + fmt::Debug + Sync + Send
{
  /// Get the source code.
  fn source(&self) -> Cow<str>;

  /// Get the source buffer.
  fn buffer(&self) -> Cow<[u8]>;

  /// Get the size of the source.
  fn size(&self) -> usize;

  /// Get the [SourceMap].
  fn map(&self, options: &MapOptions) -> Option<SourceMap>;

  /// Update hash based on the source.
  fn update_hash(&self, state: &mut dyn Hasher) {
    self.dyn_hash(state);
  }
}

impl Source for BoxSource {
  fn source(&self) -> Cow<str> {
    self.as_ref().source()
  }

  fn buffer(&self) -> Cow<[u8]> {
    self.as_ref().buffer()
  }

  fn size(&self) -> usize {
    self.as_ref().size()
  }

  fn map(&self, options: &MapOptions) -> Option<SourceMap> {
    self.as_ref().map(options)
  }
}

impl<'a> StreamChunks<'a> for BoxSource {
  fn stream_chunks(
    &'a self,
    options: &MapOptions,
    on_chunk: crate::helpers::OnChunk<'_, 'a>,
    on_source: crate::helpers::OnSource<'_, 'a>,
    on_name: crate::helpers::OnName<'_, 'a>,
  ) -> crate::helpers::GeneratedInfo {
    self
      .as_ref()
      .stream_chunks(options, on_chunk, on_source, on_name)
  }
}

// for `updateHash`
pub trait DynHash {
  fn dyn_hash(&self, state: &mut dyn Hasher);
}

impl<H: Hash> DynHash for H {
  fn dyn_hash(&self, mut state: &mut dyn Hasher) {
    self.hash(&mut state);
  }
}

impl Hash for dyn Source {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.dyn_hash(state)
  }
}

pub trait AsAny {
  fn as_any(&self) -> &dyn Any;
}

impl<T: Any> AsAny for T {
  fn as_any(&self) -> &dyn Any {
    self
  }
}

pub trait DynEq {
  fn dyn_eq(&self, other: &dyn Any) -> bool;
  fn type_id(&self) -> TypeId;
}

impl<E: Eq + Any> DynEq for E {
  fn dyn_eq(&self, other: &dyn Any) -> bool {
    if let Some(other) = other.downcast_ref::<E>() {
      self == other
    } else {
      false
    }
  }

  fn type_id(&self) -> TypeId {
    TypeId::of::<E>()
  }
}

impl PartialEq for dyn Source {
  fn eq(&self, other: &Self) -> bool {
    if self.as_any().type_id() != other.as_any().type_id() {
      return false;
    }
    self.dyn_eq(other.as_any())
  }
}

impl Eq for dyn Source {}

/// Extension methods for [Source].
pub trait SourceExt {
  /// An alias for [BoxSource::from].
  fn boxed(self) -> BoxSource;
}

impl<T: Source + 'static> SourceExt for T {
  fn boxed(self) -> BoxSource {
    Arc::new(self)
  }
}

/// Options for [Source::map].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MapOptions {
  /// Whether have columns info in generated [SourceMap] mappings.
  pub columns: bool,
  /// Whether the source will have changes, internal used for `ReplaceSource`, etc.
  pub(crate) final_source: bool,
}

impl Default for MapOptions {
  fn default() -> Self {
    Self {
      columns: true,
      final_source: false,
    }
  }
}

impl MapOptions {
  /// Create [MapOptions] with columns.
  pub fn new(columns: bool) -> Self {
    Self {
      columns,
      ..Default::default()
    }
  }
}

fn is_all_empty(val: &Arc<[String]>) -> bool {
  if val.is_empty() {
    return true;
  }
  val.iter().all(|s| s.is_empty())
}

/// The source map created by [Source::map].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceMap {
  version: u8,
  #[serde(skip_serializing_if = "Option::is_none")]
  file: Option<Arc<str>>,
  sources: Arc<[String]>,
  #[serde(rename = "sourcesContent", skip_serializing_if = "is_all_empty")]
  sources_content: Arc<[String]>,
  names: Arc<[String]>,
  mappings: Arc<str>,
  #[serde(rename = "sourceRoot", skip_serializing_if = "Option::is_none")]
  source_root: Option<Arc<str>>,
}

impl Hash for SourceMap {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.file.hash(state);
    self.mappings.hash(state);
    self.sources.hash(state);
    self.sources_content.hash(state);
    self.names.hash(state);
    self.source_root.hash(state);
  }
}

impl SourceMap {
  /// Create a [SourceMap].
  pub fn new<Mappings, Sources, SourcesContent, Names>(
    mappings: Mappings,
    sources: Sources,
    sources_content: SourcesContent,
    names: Names,
  ) -> Self
  where
    Mappings: Into<Arc<str>>,
    Sources: Into<Arc<[String]>>,
    SourcesContent: Into<Arc<[String]>>,
    Names: Into<Arc<[String]>>,
  {
    Self {
      version: 3,
      file: None,
      mappings: mappings.into(),
      sources: sources.into(),
      sources_content: sources_content.into(),
      names: names.into(),
      source_root: None,
    }
  }

  /// Get the file field in [SourceMap].
  pub fn file(&self) -> Option<&str> {
    self.file.as_deref()
  }

  /// Set the file field in [SourceMap].
  pub fn set_file<T: Into<Arc<str>>>(&mut self, file: Option<T>) {
    self.file = file.map(Into::into);
  }

  /// Get the decoded mappings in [SourceMap].
  pub fn decoded_mappings(&self) -> impl Iterator<Item = Mapping> + '_ {
    decode_mappings(self)
  }

  /// Get the mappings string in [SourceMap].
  pub fn mappings(&self) -> &str {
    &self.mappings
  }

  /// Set the mappings string in [SourceMap].
  pub fn set_mappings<T: Into<Arc<str>>>(&mut self, mappings: T) {
    self.mappings = mappings.into();
  }

  /// Get the sources field in [SourceMap].
  pub fn sources(&self) -> &[String] {
    &self.sources
  }

  /// Set the sources field in [SourceMap].
  pub fn set_sources<T: Into<Arc<[String]>>>(&mut self, sources: T) {
    self.sources = sources.into();
  }

  /// Get the source by index from sources field in [SourceMap].
  pub fn get_source(&self, index: usize) -> Option<&str> {
    self.sources.get(index).map(|s| s.as_ref())
  }

  /// Get the sourcesContent field in [SourceMap].
  pub fn sources_content(&self) -> &[String] {
    &self.sources_content
  }

  /// Set the sourcesContent field in [SourceMap].
  pub fn set_sources_content<T: Into<Arc<[String]>>>(
    &mut self,
    sources_content: T,
  ) {
    self.sources_content = sources_content.into();
  }

  /// Get the source content by index from sourcesContent field in [SourceMap].
  pub fn get_source_content(&self, index: usize) -> Option<&str> {
    self.sources_content.get(index).map(|s| s.as_ref())
  }

  /// Get the names field in [SourceMap].
  pub fn names(&self) -> &[String] {
    &self.names
  }

  /// Set the names field in [SourceMap].
  pub fn set_names<T: Into<Arc<[String]>>>(&mut self, names: T) {
    self.names = names.into();
  }

  /// Get the name by index from names field in [SourceMap].
  pub fn get_name(&self, index: usize) -> Option<&str> {
    self.names.get(index).map(|s| s.as_ref())
  }

  /// Get the source_root field in [SourceMap].
  pub fn source_root(&self) -> Option<&str> {
    self.source_root.as_deref()
  }

  /// Set the source_root field in [SourceMap].
  pub fn set_source_root<T: Into<Arc<str>>>(&mut self, source_root: Option<T>) {
    self.source_root = source_root.map(Into::into);
  }
}

#[derive(Debug, Default, Deserialize)]
struct RawSourceMap {
  pub file: Option<String>,
  pub sources: Option<Vec<Option<String>>>,
  #[serde(rename = "sourceRoot")]
  pub source_root: Option<String>,
  #[serde(rename = "sourcesContent")]
  pub sources_content: Option<Vec<Option<String>>>,
  pub names: Option<Vec<Option<String>>>,
  pub mappings: String,
}

impl RawSourceMap {
  pub fn from_reader<R: std::io::Read>(r: R) -> Result<Self> {
    let raw: RawSourceMap = simd_json::serde::from_reader(r)?;
    Ok(raw)
  }

  pub fn from_slice(val: &[u8]) -> Result<Self> {
    let mut v = val.to_vec();
    let raw: RawSourceMap = simd_json::serde::from_slice(&mut v)?;
    Ok(raw)
  }

  pub fn from_json(val: &str) -> Result<Self> {
    let mut v = val.as_bytes().to_vec();
    let raw: RawSourceMap = simd_json::serde::from_slice(&mut v)?;
    Ok(raw)
  }
}

impl SourceMap {
  /// Create a [SourceMap] from json string.
  pub fn from_json(s: &str) -> Result<Self> {
    RawSourceMap::from_json(s)?.try_into()
  }

  /// Create a [SourceMap] from [&[u8]].
  pub fn from_slice(s: &[u8]) -> Result<Self> {
    RawSourceMap::from_slice(s)?.try_into()
  }

  /// Create a [SourceMap] from reader.
  pub fn from_reader<R: std::io::Read>(s: R) -> Result<Self> {
    RawSourceMap::from_reader(s)?.try_into()
  }

  /// Generate source map to a json string.
  pub fn to_json(self) -> Result<String> {
    let json = simd_json::serde::to_string(&self)?;
    Ok(json)
  }

  /// Generate source map to writer.
  pub fn to_writer<W: std::io::Write>(self, w: W) -> Result<()> {
    simd_json::serde::to_writer(w, &self)?;
    Ok(())
  }
}

impl TryFrom<RawSourceMap> for SourceMap {
  type Error = crate::Error;

  fn try_from(raw: RawSourceMap) -> Result<Self> {
    let file = raw.file.map(Into::into);
    let mappings = raw.mappings.into();
    let sources = raw
      .sources
      .unwrap_or_default()
      .into_iter()
      .map(Option::unwrap_or_default)
      .collect::<Vec<_>>()
      .into();
    let sources_content = raw
      .sources_content
      .unwrap_or_default()
      .into_iter()
      .map(Option::unwrap_or_default)
      .collect::<Vec<_>>()
      .into();
    let names = raw
      .names
      .unwrap_or_default()
      .into_iter()
      .map(Option::unwrap_or_default)
      .collect::<Vec<_>>()
      .into();
    let source_root = raw.source_root.map(Into::into);

    Ok(Self {
      version: 3,
      file,
      mappings,
      sources,
      sources_content,
      names,
      source_root,
    })
  }
}

/// Represent a [Mapping] information of source map.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Mapping {
  /// Generated line.
  pub generated_line: u32,
  /// Generated column.
  pub generated_column: u32,
  /// Original position information.
  pub original: Option<OriginalLocation>,
}

/// Represent original position information of a [Mapping].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OriginalLocation {
  /// Source index.
  pub source_index: u32,
  /// Original line.
  pub original_line: u32,
  /// Original column.
  pub original_column: u32,
  /// Name index.
  pub name_index: Option<u32>,
}

/// An convenient way to create a [Mapping].
#[macro_export]
macro_rules! m {
  ($gl:expr, $gc:expr, $si:expr, $ol:expr, $oc:expr, $ni:expr) => {{
    let gl: i64 = $gl;
    let gc: i64 = $gc;
    let si: i64 = $si;
    let ol: i64 = $ol;
    let oc: i64 = $oc;
    let ni: i64 = $ni;
    $crate::Mapping {
      generated_line: gl as u32,
      generated_column: gc as u32,
      original: (si >= 0).then(|| $crate::OriginalLocation {
        source_index: si as u32,
        original_line: ol as u32,
        original_column: oc as u32,
        name_index: (ni >= 0).then(|| ni as u32),
      }),
    }
  }};
}

/// An convenient way to create [Mapping]s.
#[macro_export]
macro_rules! mappings {
  ($($mapping:expr),* $(,)?) => {
    ::std::vec![$({
      let mapping = $mapping;
      $crate::m![mapping[0], mapping[1], mapping[2], mapping[3], mapping[4], mapping[5]]
    }),*]
  };
}

#[cfg(test)]
mod tests {
  use std::collections::HashMap;

  use crate::{
    CachedSource, ConcatSource, OriginalSource, RawBufferSource, RawSource,
    RawStringSource, ReplaceSource, SourceMapSource, WithoutOriginalOptions,
  };

  use super::*;

  #[test]
  fn should_not_have_sources_content_field_when_it_is_empty() {
    let map = SourceMap::new(
      ";;",
      vec!["a.js".into()],
      vec!["".into(), "".into(), "".into()],
      vec!["".into(), "".into()],
    )
    .to_json()
    .unwrap();
    assert!(!map.contains("sourcesContent"));
  }

  #[test]
  fn hash_available() {
    let mut state = twox_hash::XxHash64::default();
    RawSource::from("a").hash(&mut state);
    OriginalSource::new("b", "").hash(&mut state);
    SourceMapSource::new(WithoutOriginalOptions {
      value: "c",
      name: "",
      source_map: SourceMap::from_json("{\"mappings\": \";\"}").unwrap(),
    })
    .hash(&mut state);
    ConcatSource::new([RawSource::from("d")]).hash(&mut state);
    CachedSource::new(RawSource::from("e")).hash(&mut state);
    ReplaceSource::new(RawSource::from("f")).hash(&mut state);
    RawSource::from("g").boxed().hash(&mut state);
    RawStringSource::from_static("a").hash(&mut state);
    RawBufferSource::from("a".as_bytes()).hash(&mut state);
    (&RawSource::from("h") as &dyn Source).hash(&mut state);
    ReplaceSource::new(RawSource::from("i").boxed()).hash(&mut state);
    assert_eq!(format!("{:x}", state.finish()), "f4b280bd9a8d4d3b");
  }

  #[test]
  fn eq_available() {
    assert_eq!(RawSource::from("a"), RawSource::from("a"));
    assert_eq!(
      RawStringSource::from_static("a"),
      RawStringSource::from_static("a")
    );
    assert_eq!(
      RawBufferSource::from("a".as_bytes()),
      RawBufferSource::from("a".as_bytes())
    );
    assert_eq!(OriginalSource::new("b", ""), OriginalSource::new("b", ""));
    assert_eq!(
      SourceMapSource::new(WithoutOriginalOptions {
        value: "c",
        name: "",
        source_map: SourceMap::from_json("{\"mappings\": \";\"}").unwrap(),
      }),
      SourceMapSource::new(WithoutOriginalOptions {
        value: "c",
        name: "",
        source_map: SourceMap::from_json("{\"mappings\": \";\"}").unwrap(),
      })
    );
    assert_eq!(
      ConcatSource::new([RawSource::from("d")]),
      ConcatSource::new([RawSource::from("d")])
    );
    assert_eq!(
      CachedSource::new(RawSource::from("e")),
      CachedSource::new(RawSource::from("e"))
    );
    assert_eq!(
      ReplaceSource::new(RawSource::from("f")),
      ReplaceSource::new(RawSource::from("f"))
    );
    assert_eq!(&RawSource::from("g").boxed(), &RawSource::from("g").boxed());
    assert_eq!(
      (&RawSource::from("h") as &dyn Source),
      (&RawSource::from("h") as &dyn Source)
    );
    assert_eq!(
      ReplaceSource::new(RawSource::from("i").boxed()),
      ReplaceSource::new(RawSource::from("i").boxed())
    );
    assert_eq!(
      CachedSource::new(RawSource::from("j").boxed()),
      CachedSource::new(RawSource::from("j").boxed())
    );
  }

  #[test]
  fn box_dyn_source_use_hashmap_available() {
    let mut map = HashMap::new();
    let a = RawSource::from("a").boxed();
    map.insert(a.clone(), a.clone());
    assert_eq!(map.get(&a).unwrap(), &a);
  }

  #[test]
  #[allow(suspicious_double_ref_op)]
  fn ref_dyn_source_use_hashmap_available() {
    let mut map = HashMap::new();
    let a = &RawSource::from("a") as &dyn Source;
    map.insert(a, a);
    assert_eq!(map.get(&a).unwrap(), &a);
  }
}
