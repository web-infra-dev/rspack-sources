use std::{
  borrow::Cow,
  convert::{TryFrom, TryInto},
  fmt,
  hash::{Hash, Hasher},
};

use dyn_clone::DynClone;
use serde::{Deserialize, Serialize};

use crate::{
  helpers::{decode_mappings, StreamChunks},
  Result,
};

/// An alias for [Box<dyn Source>].
pub type BoxSource = Box<dyn Source>;

/// [Source] abstraction, [webpack-sources docs](https://github.com/webpack/webpack-sources/#source).
pub trait Source:
  StreamChunks + DynHash + DynClone + fmt::Debug + Sync + Send
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

impl Source for Box<dyn Source> {
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

dyn_clone::clone_trait_object!(Source);

impl StreamChunks for Box<dyn Source> {
  fn stream_chunks(
    &self,
    options: &MapOptions,
    on_chunk: crate::helpers::OnChunk,
    on_source: crate::helpers::OnSource,
    on_name: crate::helpers::OnName,
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
    self.dyn_hash(state);
  }
}

/// Extension methods for [Source].
pub trait SourceExt {
  /// An alias for [BoxSource::from].
  fn boxed(self) -> BoxSource;
}

impl<T: Source + 'static> SourceExt for T {
  fn boxed(self) -> BoxSource {
    Box::new(self)
  }
}

/// Options for [Source::map].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MapOptions {
  /// Whether have columns info in generated [SourceMap] mappings.
  pub columns: bool,
  /// Whether the source will have changes, internal used for [ReplaceSource], etc.
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

/// The source map created by [Source::map].
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct SourceMap {
  file: Option<String>,
  mappings: String,
  sources: Vec<String>,
  sources_content: Vec<String>,
  names: Vec<String>,
}

impl SourceMap {
  /// Create a [SourceMap].
  pub fn new(
    file: Option<String>,
    mappings: String,
    sources: impl IntoIterator<Item = String>,
    sources_content: impl IntoIterator<Item = String>,
    names: impl IntoIterator<Item = String>,
  ) -> Self {
    Self {
      file,
      mappings,
      sources: sources.into_iter().collect(),
      sources_content: sources_content.into_iter().collect(),
      names: names.into_iter().collect(),
    }
  }

  /// Get the file field in [SourceMap].
  pub fn file(&self) -> Option<&str> {
    self.file.as_deref()
  }

  /// Set the file field in [SourceMap].
  pub fn set_file(&mut self, file: Option<String>) {
    self.file = file;
  }

  /// Get the decoded mappings in [SourceMap].
  pub fn decoded_mappings(&self) -> Vec<Mapping> {
    decode_mappings(self)
  }

  /// Get the mappings string in [SourceMap].
  pub fn mappings(&self) -> &str {
    &self.mappings
  }

  /// Get the sources field in [SourceMap].
  pub fn sources(&self) -> &[String] {
    &self.sources
  }

  /// Get the mutable sources field in [SourceMap].
  pub fn sources_mut(&mut self) -> &mut [String] {
    &mut self.sources
  }

  /// Get the source by index from sources field in [SourceMap].
  pub fn get_source(&self, index: usize) -> Option<&str> {
    self.sources.get(index).map(|s| s.as_str())
  }

  /// Get the mutable source by index from sources field in [SourceMap].
  pub fn get_source_mut(&mut self, index: usize) -> Option<&mut str> {
    self.sources.get_mut(index).map(|s| s.as_mut_str())
  }

  /// Get the sourcesContent field in [SourceMap].
  pub fn sources_content(&self) -> &[String] {
    &self.sources_content
  }

  /// Get the mutable sourcesContent field in [SourceMap].
  pub fn sources_content_mut(&mut self) -> &mut [String] {
    &mut self.sources_content
  }

  /// Get the source content by index from sourcesContent field in [SourceMap].
  pub fn get_source_content(&self, index: usize) -> Option<&str> {
    self.sources_content.get(index).map(|s| s.as_str())
  }

  /// Get the mutable source content by index from sourcesContent field in [SourceMap].
  pub fn get_source_content_mut(&mut self, index: usize) -> Option<&mut str> {
    self.sources_content.get_mut(index).map(|s| s.as_mut_str())
  }

  /// Get the names field in [SourceMap].
  pub fn names(&self) -> &[String] {
    &self.names
  }

  /// Get the names field in [SourceMap].
  pub fn names_mut(&mut self) -> &mut [String] {
    &mut self.names
  }

  /// Get the name by index from names field in [SourceMap].
  pub fn get_name(&self, index: usize) -> Option<&str> {
    self.names.get(index).map(|s| s.as_str())
  }

  /// Get the mutable name by index from names field in [SourceMap].
  pub fn get_name_mut(&mut self, index: usize) -> Option<&mut str> {
    self.names.get_mut(index).map(|s| s.as_mut_str())
  }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct RawSourceMap {
  pub version: Option<u8>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub file: Option<String>,
  pub sources: Option<Vec<Option<String>>>,
  #[serde(rename = "sourceRoot", skip_serializing_if = "Option::is_none")]
  pub source_root: Option<String>,
  #[serde(rename = "sourcesContent", skip_serializing_if = "Option::is_none")]
  pub sources_content: Option<Vec<Option<String>>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub names: Option<Vec<Option<String>>>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub mappings: Option<String>,
}

impl RawSourceMap {
  pub fn from_reader<R: std::io::Read>(r: R) -> Result<Self> {
    let raw: RawSourceMap = serde_json::from_reader(r)?;
    Ok(raw)
  }

  pub fn from_slice(v: &[u8]) -> Result<Self> {
    let raw: RawSourceMap = serde_json::from_slice(v)?;
    Ok(raw)
  }

  pub fn from_json(s: &str) -> Result<Self> {
    let raw: RawSourceMap = serde_json::from_str(s)?;
    Ok(raw)
  }

  pub fn to_json(&self) -> Result<String> {
    let json = serde_json::to_string(self)?;
    Ok(json)
  }

  pub fn to_writer<W: std::io::Write>(&self, w: W) -> Result<()> {
    serde_json::to_writer(w, self)?;
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
    let sources = raw.sources.unwrap_or_default();
    let sources = match raw.source_root {
      Some(ref source_root) if !source_root.is_empty() => {
        let source_root = source_root.trim_end_matches('/');
        sources
          .into_iter()
          .map(|x| {
            let x = x.unwrap_or_default();
            let is_valid = !x.is_empty()
              && (x.starts_with('/')
                || x.starts_with("http:")
                || x.starts_with("https:"));
            if is_valid {
              x
            } else {
              format!("{}/{}", source_root, x)
            }
          })
          .collect()
      }
      _ => sources.into_iter().map(Option::unwrap_or_default).collect(),
    };
    let sources_content = raw
      .sources_content
      .unwrap_or_default()
      .into_iter()
      .map(|v| v.unwrap_or_default())
      .collect();
    let names = raw
      .names
      .unwrap_or_default()
      .into_iter()
      .map(|v| v.unwrap_or_default())
      .collect();
    Ok(Self {
      file: raw.file,
      mappings: raw.mappings.unwrap_or_default(),
      sources,
      sources_content,
      names,
    })
  }
}

impl From<SourceMap> for RawSourceMap {
  fn from(map: SourceMap) -> Self {
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
      source_root: None,
      sources_content: Some(
        map
          .sources_content
          .into_iter()
          .map(|s| (!s.is_empty()).then_some(s))
          .collect(),
      ),
      names: Some(
        map
          .names
          .into_iter()
          .map(|s| (!s.is_empty()).then_some(s))
          .collect(),
      ),
      mappings: (!map.mappings.is_empty()).then_some(map.mappings),
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
