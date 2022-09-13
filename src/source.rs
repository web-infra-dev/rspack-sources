use std::{
  borrow::Cow,
  convert::{TryFrom, TryInto},
  fmt,
  hash::{Hash, Hasher},
};

use serde::{Deserialize, Serialize};

use crate::{
  helpers::{decode_mappings, StreamChunks},
  Result,
};

pub type BoxSource = Box<dyn Source>;

pub trait Source: StreamChunks + DynHash + fmt::Debug + Sync + Send {
  fn source(&self) -> Cow<str>;

  fn buffer(&self) -> Cow<[u8]>;

  fn size(&self) -> usize;

  fn map(&self, options: &MapOptions) -> Option<SourceMap>;

  fn update_hash(&self, state: &mut dyn Hasher) {
    self.dyn_hash(state);
  }
}

impl<T: Source + 'static> From<T> for BoxSource {
  fn from(s: T) -> Self {
    Box::new(s)
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

pub trait SourceExt {
  fn boxed(self) -> BoxSource;
}

impl<T: Source + 'static> SourceExt for T {
  fn boxed(self) -> BoxSource {
    self.into()
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MapOptions {
  pub columns: bool,
  pub final_source: bool,
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
  pub fn new(columns: bool) -> Self {
    Self {
      columns,
      ..Default::default()
    }
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SourceMap {
  file: Option<String>,
  mappings: String,
  sources: Vec<String>,
  sources_content: Vec<String>,
  names: Vec<String>,
}

impl SourceMap {
  pub fn new<S: Into<String>>(
    file: Option<S>,
    mappings: S,
    sources: impl IntoIterator<Item = S>,
    sources_content: impl IntoIterator<Item = S>,
    names: impl IntoIterator<Item = S>,
  ) -> Self {
    Self {
      file: file.map(Into::into),
      mappings: mappings.into(),
      sources: sources.into_iter().map(|s| s.into()).collect(),
      sources_content: sources_content.into_iter().map(|s| s.into()).collect(),
      names: names.into_iter().map(|s| s.into()).collect(),
    }
  }

  pub fn file(&self) -> Option<&str> {
    self.file.as_deref()
  }

  pub fn set_file(&mut self, file: Option<String>) {
    self.file = file;
  }

  pub fn decoded_mappings(&self) -> Vec<Mapping> {
    decode_mappings(self)
  }

  pub fn mappings(&self) -> &str {
    &self.mappings
  }

  pub fn sources(&self) -> &[String] {
    &self.sources
  }

  pub fn get_source(&self, index: usize) -> Option<&str> {
    self.sources.get(index).map(|s| s.as_str())
  }

  pub fn sources_content(&self) -> &[String] {
    &self.sources_content
  }

  pub fn get_source_content(&self, index: usize) -> Option<&str> {
    self.sources_content.get(index).map(|s| s.as_str())
  }

  pub fn names(&self) -> &[String] {
    &self.names
  }

  pub fn get_name(&self, index: usize) -> Option<&str> {
    self.names.get(index).map(|s| s.as_str())
  }
}

#[derive(Serialize, Deserialize)]
pub struct RawSourceMap {
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
  pub fn from_json(s: &str) -> Result<Self> {
    RawSourceMap::from_json(s)?.try_into()
  }

  pub fn from_slice(s: &[u8]) -> Result<Self> {
    RawSourceMap::from_slice(s)?.try_into()
  }

  pub fn from_reader<R: std::io::Read>(s: R) -> Result<Self> {
    RawSourceMap::from_reader(s)?.try_into()
  }

  pub fn to_json(&self) -> Result<String> {
    let raw = RawSourceMap::from(self.clone());
    raw.to_json()
  }

  pub fn to_writer<W: std::io::Write>(&self, w: W) -> Result<()> {
    let raw = RawSourceMap::from(self.clone());
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Mapping {
  pub generated_line: u32,
  pub generated_column: u32,
  pub original: Option<OriginalLocation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OriginalLocation {
  pub source_index: u32,
  pub original_line: u32,
  pub original_column: u32,
  pub name_index: Option<u32>,
}

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

#[macro_export]
macro_rules! mappings {
  ($($mapping:expr),* $(,)?) => {
    ::std::vec![$({
      let mapping = $mapping;
      $crate::m![mapping[0], mapping[1], mapping[2], mapping[3], mapping[4], mapping[5]]
    }),*]
  };
}
