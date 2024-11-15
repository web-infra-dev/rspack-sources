use std::{
  any::{Any, TypeId},
  borrow::Cow,
  convert::{TryFrom, TryInto},
  fmt::Debug,
  hash::{Hash, Hasher},
  sync::Arc,
};

use dyn_clone::DynClone;
use serde::{Deserialize, Serialize};

use crate::{
  helpers::{decode_mappings, StreamChunks},
  Result,
};

/// An alias for `Box<dyn Source>`.
pub type BoxSource = Arc<dyn Source>;

/// [Source] abstraction, [webpack-sources docs](https://github.com/webpack/webpack-sources/#source).
pub trait Source:
  for<'a> StreamChunks<'a>
  + DynHash
  + AsAny
  + DynEq
  + DynClone
  + Debug
  + Sync
  + Send
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

  /// Writes the source into a writer, preferably a `std::io::BufWriter<std::io::Write>`.
  fn to_writer(&self, writer: &mut dyn std::io::Write) -> std::io::Result<()>;
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

  fn to_writer(&self, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
    self.as_ref().to_writer(writer)
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

/// The `DecodableMap` trait provides function for obtaining the decoded mappings
pub trait DecodableMap: Sync + Send {
  /// Get the file field in [SourceMap].
  fn file(&self) -> Option<&str>;

  /// Get the decoded mappings in [SourceMap].
  fn decoded_mappings<'a>(&'a self) -> Box<dyn Iterator<Item = Mapping> + 'a>;

  /// Get the mappings string in [SourceMap].
  fn mappings(&self) -> &str;

  /// Get the sources field in [SourceMap].
  fn sources<'a>(&'a self) -> Box<dyn Iterator<Item = &'a str> + 'a>;

  /// Get the sources field in [SourceMap].
  fn source(&self, index: usize) -> Option<&str>;

  /// Get the sourcesContent field in [SourceMap].
  fn sources_content<'a>(&'a self) -> Box<dyn Iterator<Item = &'a str> + 'a>;

  /// Get the sourcesContent field in [SourceMap].
  fn source_content(&self, index: usize) -> Option<&str>;

  /// Get the names field in [SourceMap].
  fn names<'a>(&'a self) -> Box<dyn Iterator<Item = &'a str> + 'a>;

  /// Get the names field in [SourceMap].
  fn name(&self, index: usize) -> Option<&str>;

  /// Get the source_root field in [SourceMap].
  fn source_root(&self) -> Option<&str>;

  /// Generate source map to a json string.
  fn to_json(self: Box<Self>) -> Result<String>;
}

pub type BoxDecodableMap = Box<dyn DecodableMap>;

impl PartialEq for BoxDecodableMap {
  fn eq(&self, other: &Self) -> bool {
    self.file() == other.file()
      && self.mappings() == other.mappings()
      && self.sources().collect::<Vec<&str>>()
        == other.sources().collect::<Vec<&str>>()
      && self.sources_content().collect::<Vec<&str>>()
        == other.sources_content().collect::<Vec<&str>>()
      && self.names().collect::<Vec<&str>>()
        == other.names().collect::<Vec<&str>>()
      && self.source_root() == other.source_root()
  }
}

impl Eq for BoxDecodableMap {}

impl Clone for BoxDecodableMap {
  fn clone(&self) -> Self {
    Box::new(SourceMap::new(
      self.file().map(|file| Arc::from(file.to_string())),
      Arc::from(self.mappings().to_string()),
      self.sources().map(Arc::from).collect::<Vec<_>>(),
      self.sources_content().map(Arc::from).collect::<Vec<_>>(),
      self.names().map(Arc::from).collect::<Vec<_>>(),
    ))
  }
}

impl Hash for BoxDecodableMap {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.file().hash(state);
    self.mappings().hash(state);
    self.sources().for_each(|source| source.hash(state));
    self
      .sources_content()
      .for_each(|content| content.hash(state));
    self.names().for_each(|name| name.hash(state));
    self.source_root().hash(state);
  }
}

impl Debug for BoxDecodableMap {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("BoxDecodableMap")
      .field("file", &self.file())
      .finish()
  }
}

impl<T: DecodableMap + 'static> From<T> for BoxDecodableMap {
  fn from(value: T) -> Self {
    Box::new(value)
  }
}

pub trait DecodableMapExt {
  /// An alias for [BoxDecodableMap::from].
  fn boxed(self) -> BoxDecodableMap;
}

impl<T: DecodableMap + 'static> DecodableMapExt for T {
  fn boxed(self) -> BoxDecodableMap {
    Box::new(self)
  }
}

/// The source map created by [Source::map].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMap {
  file: Option<Arc<str>>,
  mappings: Arc<str>,
  sources: Vec<Arc<str>>,
  sources_content: Vec<Arc<str>>,
  names: Vec<Arc<str>>,
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
  pub fn new(
    file: Option<Arc<str>>,
    mappings: Arc<str>,
    sources: Vec<Arc<str>>,
    sources_content: Vec<Arc<str>>,
    names: Vec<Arc<str>>,
  ) -> Self {
    Self {
      file,
      mappings,
      sources,
      sources_content,
      names,
      source_root: None,
    }
  }

  /// Get the file field in [SourceMap].
  pub fn file(&self) -> &Option<Arc<str>> {
    &self.file
  }

  /// Set the file field in [SourceMap].
  pub fn file_mut(&mut self) -> &mut Option<Arc<str>> {
    &mut self.file
  }

  /// Get the mappings string in [SourceMap].
  pub fn mappings(&self) -> &Arc<str> {
    &self.mappings
  }

  /// Get the sources field in [SourceMap].
  pub fn sources(&self) -> &[Arc<str>] {
    &self.sources
  }

  /// Set sources field in [SourceMap].
  pub fn sources_mut(&mut self) -> &mut Vec<Arc<str>> {
    &mut self.sources
  }

  /// Get the sourcesContent field in [SourceMap].
  pub fn sources_content(&self) -> &[Arc<str>] {
    &self.sources_content
  }

  pub fn sources_content_mut(&mut self) -> &mut Vec<Arc<str>> {
    &mut self.sources_content
  }

  /// Get the names field in [SourceMap].
  pub fn names(&self) -> &[Arc<str>] {
    &self.names
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
  pub fn source_root_mut(&mut self) -> &mut Option<Arc<str>> {
    &mut self.source_root
  }
}

impl DecodableMap for SourceMap {
  fn file(&self) -> Option<&str> {
    self.file.as_deref()
  }

  fn decoded_mappings<'a>(&'a self) -> Box<dyn Iterator<Item = Mapping> + 'a> {
    Box::new(decode_mappings(&self.mappings))
  }

  fn mappings(&self) -> &str {
    self.mappings.as_ref()
  }

  fn sources<'a>(&'a self) -> Box<dyn Iterator<Item = &'a str> + 'a> {
    Box::new(self.sources.iter().map(|source| source.as_ref()))
  }

  fn source(&self, index: usize) -> Option<&str> {
    self.sources.get(index).map(|source| source.as_ref())
  }

  fn sources_content<'a>(&'a self) -> Box<dyn Iterator<Item = &'a str> + 'a> {
    Box::new(self.sources_content.iter().map(|content| content.as_ref()))
  }

  fn source_content(&self, index: usize) -> Option<&str> {
    self
      .sources_content
      .get(index)
      .map(|content| content.as_ref())
  }

  fn names<'a>(&'a self) -> Box<dyn Iterator<Item = &'a str> + 'a> {
    Box::new(self.names.iter().map(|name| name.as_ref()))
  }

  fn name(&self, index: usize) -> Option<&str> {
    self.names.get(index).map(|name| name.as_ref())
  }

  fn source_root(&self) -> Option<&str> {
    self.source_root.as_deref()
  }

  fn to_json(self: Box<Self>) -> Result<String> {
    let raw = RawSourceMap::from(*self);
    raw.to_json()
  }
}

#[derive(Debug, Serialize, Deserialize)]
struct RawSourceMap {
  pub version: Option<u8>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub file: Option<Arc<str>>,
  pub sources: Option<Vec<Option<Arc<str>>>>,
  #[serde(rename = "sourceRoot", skip_serializing_if = "Option::is_none")]
  pub source_root: Option<Arc<str>>,
  #[serde(rename = "sourcesContent", skip_serializing_if = "Option::is_none")]
  pub sources_content: Option<Vec<Option<Arc<str>>>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub names: Option<Vec<Option<Arc<str>>>>,
  pub mappings: Arc<str>,
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

  pub fn to_json(&self) -> Result<String> {
    let json = simd_json::serde::to_string(self)?;
    Ok(json)
  }

  pub fn to_writer<W: std::io::Write>(&self, w: W) -> Result<()> {
    simd_json::to_writer(w, self)?;
    Ok(())
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
    let raw = RawSourceMap::from(self);
    raw.to_json()
  }

  /// Generate source map to writer.
  pub fn to_writer<W: std::io::Write>(self, w: W) -> Result<()> {
    let raw = RawSourceMap::from(self);
    raw.to_writer(w)
  }
}

impl TryFrom<RawSourceMap> for SourceMap {
  type Error = crate::Error;

  fn try_from(raw: RawSourceMap) -> Result<Self> {
    let sources = raw
      .sources
      .unwrap_or_default()
      .into_iter()
      .map(|source| match source {
        Some(s) => s,
        None => Arc::from(""),
      })
      .collect();
    let sources_content = raw
      .sources_content
      .unwrap_or_default()
      .into_iter()
      .map(|content| match content {
        Some(c) => c,
        None => Arc::from(""),
      })
      .map(Arc::from)
      .collect();
    let names = raw
      .names
      .unwrap_or_default()
      .into_iter()
      .map(|name| match name {
        Some(n) => n,
        None => Arc::from(""),
      })
      .map(Arc::from)
      .collect();
    Ok(Self {
      file: raw.file,
      mappings: raw.mappings,
      sources,
      sources_content,
      names,
      source_root: raw.source_root,
    })
  }
}

impl From<SourceMap> for RawSourceMap {
  fn from(map: SourceMap) -> Self {
    let sources_content = map
      .sources_content
      .into_iter()
      .map(|s| (!s.is_empty()).then_some(s));
    Self {
      version: Some(3),
      file: map.file,
      sources: Some(
        map
          .sources
          .into_iter()
          .map(|s| (!s.is_empty()).then_some(s))
          .collect(),
      ),
      source_root: map.source_root,
      sources_content: sources_content
        .clone()
        .any(|s| s.is_some())
        .then(|| sources_content.collect()),
      names: Some(
        map
          .names
          .into_iter()
          .map(|s| (!s.is_empty()).then_some(s))
          .collect(),
      ),
      mappings: map.mappings,
    }
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
    CachedSource, ConcatSource, OriginalSource, RawSource, ReplaceSource,
    SourceMapSource, WithoutOriginalOptions,
  };

  use super::*;

  #[test]
  fn should_not_have_sources_content_field_when_it_is_empty() {
    let map = SourceMap::new(
      None,
      Arc::from(";;"),
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
    (&RawSource::from("h") as &dyn Source).hash(&mut state);
    ReplaceSource::new(RawSource::from("i")).hash(&mut state);
    assert_eq!(format!("{:x}", state.finish()), "537e65c8b3655fc6");
  }

  #[test]
  fn eq_available() {
    assert_eq!(RawSource::from("a"), RawSource::from("a"));
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
  #[allow(clippy::clone_double_ref)]
  fn clone_available() {
    let a = RawSource::from("a");
    assert_eq!(a, a.clone());
    let b = OriginalSource::new("b", "");
    assert_eq!(b, b.clone());
    let c = SourceMapSource::new(WithoutOriginalOptions {
      value: "c",
      name: "",
      source_map: SourceMap::from_json("{\"mappings\": \";\"}").unwrap(),
    });
    assert_eq!(c, c.clone());
    let d = ConcatSource::new([RawSource::from("d")]);
    assert_eq!(d, d.clone());
    let e = CachedSource::new(RawSource::from("e"));
    assert_eq!(e, e.clone());
    let f = ReplaceSource::new(RawSource::from("f"));
    assert_eq!(f, f.clone());
    let g = RawSource::from("g").boxed();
    assert_eq!(&g, &g.clone());
    let h = &RawSource::from("h") as &dyn Source;
    assert_eq!(h, h);
    let i = ReplaceSource::new(RawSource::from("i").boxed());
    assert_eq!(i, i.clone());
    let j = CachedSource::new(RawSource::from("j").boxed());
    assert_eq!(j, j.clone());
  }

  #[test]
  fn box_dyn_source_use_hashmap_available() {
    let mut map = HashMap::new();
    let a = RawSource::from("a").boxed();
    map.insert(a.clone(), a.clone());
    assert_eq!(map.get(&a).unwrap(), &a);
  }

  #[test]
  #[allow(clippy::clone_double_ref)]
  fn ref_dyn_source_use_hashmap_available() {
    let mut map = HashMap::new();
    let a = &RawSource::from("a") as &dyn Source;
    map.insert(a, a);
    assert_eq!(map.get(&a).unwrap(), &a);
  }

  #[test]
  fn to_writer() {
    let sources =
      ConcatSource::new([RawSource::from("a"), RawSource::from("b")]);
    let mut writer = std::io::BufWriter::new(Vec::new());
    let result = sources.to_writer(&mut writer);
    assert!(result.is_ok());
    assert_eq!(
      String::from_utf8(writer.into_inner().unwrap()).unwrap(),
      "ab"
    );
  }
}
