use std::{
  hash::{Hash, Hasher},
  sync::Arc,
};

use serde::Serialize;
use simd_json::{
  base::ValueAsScalar,
  derived::{ValueObjectAccessAsArray, ValueObjectAccessAsScalar},
  BorrowedValue,
};

use crate::{helpers::decode_mappings, Mapping, Result};

fn is_all_owned_empty(val: &Arc<[String]>) -> bool {
  if val.is_empty() {
    return true;
  }
  val.iter().all(|s| s.is_empty())
}

/// The source map created by [Source::map].
#[derive(Clone, PartialEq, Eq, Serialize, Hash)]
pub struct OwnedSourceMap {
  version: u8,
  #[serde(skip_serializing_if = "Option::is_none")]
  file: Option<Arc<str>>,
  sources: Arc<[String]>,
  #[serde(
    rename = "sourcesContent",
    skip_serializing_if = "is_all_owned_empty"
  )]
  sources_content: Arc<[String]>,
  names: Arc<[String]>,
  mappings: Arc<str>,
  #[serde(rename = "sourceRoot", skip_serializing_if = "Option::is_none")]
  source_root: Option<Arc<str>>,
  #[serde(rename = "debugId", skip_serializing_if = "Option::is_none")]
  debug_id: Option<Arc<str>>,
  #[serde(rename = "ignoreList", skip_serializing_if = "Option::is_none")]
  ignore_list: Option<Arc<Vec<u32>>>,
}

impl OwnedSourceMap {
  pub fn to_json(&self) -> Result<String> {
    let json = simd_json::serde::to_string(&self)?;
    Ok(json)
  }

  pub fn to_writer<W: std::io::Write>(&self, w: W) -> Result<()> {
    simd_json::serde::to_writer(w, self)?;
    Ok(())
  }
}

fn is_all_borrowed_empty(val: &[&str]) -> bool {
  if val.is_empty() {
    return true;
  }
  val.iter().all(|s| s.is_empty())
}

#[derive(Clone, PartialEq, Eq, Serialize, Hash)]
struct BorrowedSourceMap<'a> {
  version: u8,
  #[serde(skip_serializing_if = "Option::is_none")]
  file: Option<&'a str>,
  sources: Vec<&'a str>,
  #[serde(
    rename = "sourcesContent",
    skip_serializing_if = "is_all_borrowed_empty"
  )]
  sources_content: Vec<&'a str>,
  names: Vec<&'a str>,
  mappings: &'a str,
  #[serde(rename = "sourceRoot", skip_serializing_if = "Option::is_none")]
  source_root: Option<&'a str>,
  #[serde(rename = "debugId", skip_serializing_if = "Option::is_none")]
  debug_id: Option<&'a str>,
  #[serde(rename = "ignoreList", skip_serializing_if = "Option::is_none")]
  ignore_list: Option<Arc<Vec<u32>>>,
}

impl PartialEq<OwnedSourceMap> for BorrowedSourceMap<'_> {
  fn eq(&self, other: &OwnedSourceMap) -> bool {
    self.file == other.file.as_deref()
      && self.mappings == other.mappings.as_ref()
      && self.sources == other.sources.as_ref()
      && self.sources_content == other.sources_content.as_ref()
      && self.names == other.names.as_ref()
      && self.source_root == other.source_root.as_deref()
      && self.ignore_list == other.ignore_list
  }
}

impl BorrowedSourceMap<'_> {
  fn to_json(&self) -> Result<String> {
    let json = simd_json::serde::to_string(&self)?;
    Ok(json)
  }

  pub fn to_writer<W: std::io::Write>(&self, w: W) -> Result<()> {
    simd_json::serde::to_writer(w, self)?;
    Ok(())
  }
}

type Owner = Vec<u8>;

self_cell::self_cell!(
  struct BorrowedValueCell {
    owner: Owner,

    #[covariant]
    dependent: BorrowedValue,
  }
);

self_cell::self_cell!(
  struct StaticSourceMap {
    owner: Arc<BorrowedValueCell>,

    #[covariant]
    dependent: BorrowedSourceMap,
  }
);

impl PartialEq for StaticSourceMap {
  fn eq(&self, other: &Self) -> bool {
    self.borrow_dependent() == other.borrow_dependent()
  }
}

impl Eq for StaticSourceMap {}

impl Hash for StaticSourceMap {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.borrow_dependent().hash(state);
  }
}

impl StaticSourceMap {
  fn from_borrowed_value_cell(cell: Arc<BorrowedValueCell>) -> Self {
    Self::new(cell, |owner| BorrowedSourceMap {
      version: 3,
      file: owner.borrow_dependent().get_str("file").map(Into::into),
      sources: owner
        .borrow_dependent()
        .get_array("sources")
        .map(|v| {
          v.iter()
            .map(|s| s.as_str().unwrap_or_default())
            .collect::<Vec<_>>()
        })
        .unwrap_or_default(),
      sources_content: owner
        .borrow_dependent()
        .get_array("sourcesContent")
        .map(|v| {
          v.iter()
            .map(|s| s.as_str().unwrap_or_default())
            .collect::<Vec<_>>()
        })
        .unwrap_or_default(),
      names: owner
        .borrow_dependent()
        .get_array("names")
        .map(|v| {
          v.iter()
            .map(|s| s.as_str().unwrap_or_default())
            .collect::<Vec<_>>()
        })
        .unwrap_or_default(),
      mappings: owner
        .borrow_dependent()
        .get_str("mappings")
        .unwrap_or_default(),
      source_root: owner
        .borrow_dependent()
        .get_str("sourceRoot")
        .map(Into::into),
      debug_id: owner.borrow_dependent().get_str("debugId").map(Into::into),
      ignore_list: owner.borrow_dependent().get_array("ignoreList").map(|v| {
        v.iter()
          .map(|n| n.as_u32().unwrap_or_default())
          .collect::<Vec<_>>()
          .into()
      }),
    })
  }

  pub fn from_json(json: String) -> Result<Self> {
    Self::from_slice(json.into_bytes())
  }

  pub fn from_slice(slice: Vec<u8>) -> Result<Self> {
    let borrowed_value_cell = BorrowedValueCell::try_new(slice, |owner| {
      // We need a mutable slice from our owned data
      // SAFETY: We're creating a mutable reference to the owned data.
      // The self_cell ensures this reference is valid for the lifetime of the cell.
      #[allow(unsafe_code)]
      let bytes: &'static mut [u8] = unsafe {
        std::slice::from_raw_parts_mut(owner.as_ptr().cast_mut(), owner.len())
      };
      simd_json::to_borrowed_value(bytes)
    })?;
    Ok(Self::from_borrowed_value_cell(Arc::new(
      borrowed_value_cell,
    )))
  }

  pub fn to_json(&self) -> Result<String> {
    self.borrow_dependent().to_json()
  }

  pub fn to_writer<W: std::io::Write>(&self, w: W) -> Result<()> {
    self.borrow_dependent().to_writer(w)
  }
}

#[derive(Clone, Eq)]
enum SourceMapCell {
  Static(Arc<StaticSourceMap>),
  Owned(OwnedSourceMap),
}

impl PartialEq for SourceMapCell {
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (SourceMapCell::Static(this), SourceMapCell::Static(other)) => {
        this.borrow_dependent() == other.borrow_dependent()
      }
      (SourceMapCell::Static(this), SourceMapCell::Owned(other)) => {
        this.borrow_dependent() == other
      }
      (SourceMapCell::Owned(this), SourceMapCell::Static(other)) => {
        other.borrow_dependent() == this
      }
      (SourceMapCell::Owned(this), SourceMapCell::Owned(other)) => {
        this == other
      }
    }
  }
}

impl Hash for SourceMapCell {
  fn hash<H: Hasher>(&self, state: &mut H) {
    match self {
      SourceMapCell::Static(s) => s.hash(state),
      SourceMapCell::Owned(owned) => owned.hash(state),
    }
  }
}

/// Source map representation and utilities.
///
/// This struct serves multiple purposes in the source mapping ecosystem:
///
/// 1. **Source Map Generation**: Created by the `map()` method of various `Source`
///    implementations to provide mapping information between generated and original code
///
/// 2. **JSON Deserialization**: Can be constructed from JSON strings via `from_json()`,
///    enabling integration with external source map files and `SourceMapSource` usage
///
/// 3. **Caching Optimization**: Used by `CachedSource` to store computed source maps,
///    preventing expensive recomputation of mapping data during repeated access
///
/// The source map follows the [Source Map Specification v3](https://docs.google.com/document/d/1U1RGAehQwRypUTovF1KRlpiOFze0b-_2gc6fAH0KY0k/edit)
/// and provides efficient serialization/deserialization capabilities.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct SourceMap(SourceMapCell);

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
    Self(SourceMapCell::Owned(OwnedSourceMap {
      version: 3,
      file: None,
      mappings: mappings.into(),
      sources: sources.into(),
      sources_content: sources_content.into(),
      names: names.into(),
      source_root: None,
      debug_id: None,
      ignore_list: None,
    }))
  }

  fn ensure_owned(&mut self) -> &mut OwnedSourceMap {
    if matches!(self.0, SourceMapCell::Static(_)) {
      let cell = match &self.0 {
        SourceMapCell::Static(s) => s.with_dependent(|_, dependent| {
          SourceMapCell::Owned(OwnedSourceMap {
            version: dependent.version,
            file: dependent.file.map(|s| s.to_string().into()),
            sources: dependent
              .sources
              .iter()
              .map(|s| s.to_string())
              .collect::<Vec<_>>()
              .into(),
            sources_content: dependent
              .sources_content
              .iter()
              .map(|s| s.to_string())
              .collect::<Vec<_>>()
              .into(),
            names: dependent
              .names
              .iter()
              .map(|s| s.to_string())
              .collect::<Vec<_>>()
              .into(),
            mappings: dependent.mappings.to_string().into(),
            source_root: dependent.source_root.map(|s| s.to_string().into()),
            debug_id: dependent.debug_id.map(|s| s.to_string().into()),
            ignore_list: dependent.ignore_list.clone(),
          })
        }),
        SourceMapCell::Owned(_) => unreachable!(),
      };
      self.0 = cell;
    }
    match &mut self.0 {
      SourceMapCell::Static(_) => {
        unreachable!()
      }
      SourceMapCell::Owned(owned) => owned,
    }
  }

  /// Get the file field in [SourceMap].
  pub fn file(&self) -> Option<&str> {
    match &self.0 {
      SourceMapCell::Static(s) => s.borrow_dependent().file.as_ref().copied(),
      SourceMapCell::Owned(owned) => owned.file.as_ref().map(|s| s.as_ref()),
    }
  }

  /// Set the file field in [SourceMap].
  pub fn set_file<T: Into<Arc<str>>>(&mut self, file: Option<T>) {
    self.ensure_owned().file = file.map(Into::into);
  }

  /// Get the ignoreList field in [SourceMap].
  pub fn ignore_list(&self) -> Option<&Vec<u32>> {
    match &self.0 {
      SourceMapCell::Static(s) => s.borrow_dependent().ignore_list.as_deref(),
      SourceMapCell::Owned(owned) => owned.ignore_list.as_deref(),
    }
  }

  /// Set the ignoreList field in [SourceMap].
  pub fn set_ignore_list<T: Into<Arc<Vec<u32>>>>(
    &mut self,
    ignore_list: Option<T>,
  ) {
    self.ensure_owned().ignore_list = ignore_list.map(Into::into);
  }

  /// Get the decoded mappings in [SourceMap].
  pub fn decoded_mappings(&self) -> impl Iterator<Item = Mapping> + '_ {
    decode_mappings(self)
  }

  /// Get the mappings string in [SourceMap].
  pub fn mappings(&self) -> &str {
    match &self.0 {
      SourceMapCell::Static(s) => s.borrow_dependent().mappings,
      SourceMapCell::Owned(owned) => owned.mappings.as_ref(),
    }
  }

  /// Get the sources field in [SourceMap].
  pub fn sources(&self) -> Box<dyn Iterator<Item = &str> + Send + '_> {
    match &self.0 {
      SourceMapCell::Static(s) => {
        Box::new(s.borrow_dependent().sources.iter().copied())
      }
      SourceMapCell::Owned(owned) => {
        Box::new(owned.sources.iter().map(|s| s.as_str()))
      }
    }
  }

  /// Set the sources field in [SourceMap].
  pub fn set_sources<T: Into<Arc<[String]>>>(&mut self, sources: T) {
    self.ensure_owned().sources = sources.into()
  }

  /// Get the source by index from sources field in [SourceMap].
  pub fn get_source(&self, index: usize) -> Option<&str> {
    match &self.0 {
      SourceMapCell::Static(s) => {
        s.borrow_dependent().sources.get(index).copied()
      }
      SourceMapCell::Owned(owned) => {
        owned.sources.get(index).map(AsRef::as_ref)
      }
    }
  }

  /// Get the sourcesContent field in [SourceMap].
  pub fn sources_content(&self) -> Box<dyn Iterator<Item = &str> + Send + '_> {
    match &self.0 {
      SourceMapCell::Static(s) => {
        Box::new(s.borrow_dependent().sources_content.iter().copied())
      }
      SourceMapCell::Owned(borrowed) => {
        Box::new(borrowed.sources_content.iter().map(|s| s.as_str()))
      }
    }
  }

  /// Set the sourcesContent field in [SourceMap].
  pub fn set_sources_content<T: Into<Arc<[String]>>>(
    &mut self,
    sources_content: T,
  ) {
    self.ensure_owned().sources_content = sources_content.into()
  }

  /// Get the source content by index from sourcesContent field in [SourceMap].
  pub fn get_source_content(&self, index: usize) -> Option<&str> {
    match &self.0 {
      SourceMapCell::Static(s) => {
        s.borrow_dependent().sources_content.get(index).copied()
      }
      SourceMapCell::Owned(owned) => {
        owned.sources_content.get(index).map(AsRef::as_ref)
      }
    }
  }

  /// Get the names field in [SourceMap].
  pub fn names(&self) -> Box<dyn Iterator<Item = &str> + Send + '_> {
    match &self.0 {
      SourceMapCell::Static(s) => {
        Box::new(s.borrow_dependent().names.iter().copied())
      }
      SourceMapCell::Owned(owned) => {
        Box::new(owned.names.iter().map(|s| s.as_str()))
      }
    }
  }

  /// Set the names field in [SourceMap].
  pub fn set_names<T: Into<Arc<[String]>>>(&mut self, names: Vec<String>) {
    self.ensure_owned().names = names.into();
  }

  /// Get the name by index from names field in [SourceMap].
  pub fn get_name(&self, index: usize) -> Option<&str> {
    match &self.0 {
      SourceMapCell::Static(s) => {
        s.borrow_dependent().names.get(index).copied()
      }
      SourceMapCell::Owned(owned) => owned.names.get(index).map(AsRef::as_ref),
    }
  }

  /// Get the source_root field in [SourceMap].
  pub fn source_root(&self) -> Option<&str> {
    match &self.0 {
      SourceMapCell::Static(s) => {
        s.borrow_dependent().source_root.as_ref().map(|s| *s)
      }
      SourceMapCell::Owned(owned) => {
        owned.source_root.as_ref().map(|s| s.as_ref())
      }
    }
  }

  /// Set the source_root field in [SourceMap].
  pub fn set_source_root<T: Into<Arc<str>>>(&mut self, source_root: Option<T>) {
    self.ensure_owned().source_root = source_root.map(Into::into);
  }

  /// Set the debug_id field in [SourceMap].
  pub fn set_debug_id<T: Into<Arc<str>>>(&mut self, debug_id: Option<T>) {
    self.ensure_owned().debug_id = debug_id.map(Into::into);
  }

  /// Get the debug_id field in [SourceMap].
  pub fn get_debug_id(&self) -> Option<&str> {
    match &self.0 {
      SourceMapCell::Static(s) => {
        s.borrow_dependent().debug_id.as_ref().map(|s| *s)
      }
      SourceMapCell::Owned(owned) => {
        owned.debug_id.as_ref().map(|s| s.as_ref())
      }
    }
  }
}

impl std::fmt::Debug for SourceMap {
  fn fmt(
    &self,
    f: &mut std::fmt::Formatter<'_>,
  ) -> std::result::Result<(), std::fmt::Error> {
    let indent = f.width().unwrap_or(0);
    let indent_str = format!("{:indent$}", "", indent = indent);

    write!(
      f,
      "{indent_str}SourceMap::from_json({:?}.to_string()).unwrap()",
      self.to_json().unwrap()
    )?;

    Ok(())
  }
}

impl SourceMap {
  /// Create a [SourceMap] from json string.
  pub fn from_json(json: impl Into<String>) -> Result<SourceMap> {
    let s = StaticSourceMap::from_json(json.into())?;
    Ok(SourceMap(SourceMapCell::Static(s.into())))
  }

  /// Create a [SourceMap] from reader.
  pub fn from_reader<R: std::io::Read>(mut s: R) -> Result<Self> {
    let mut json = String::default();
    s.read_to_string(&mut json)?;
    Self::from_json(json)
  }

  /// Creates a [SourceMap] from a byte slice containing JSON data.
  pub fn from_slice(slice: impl Into<Vec<u8>>) -> Result<SourceMap> {
    let s = StaticSourceMap::from_slice(slice.into())?;
    Ok(SourceMap(SourceMapCell::Static(s.into())))
  }

  /// Generate source map to a json string.
  pub fn to_json(&self) -> Result<String> {
    match &self.0 {
      SourceMapCell::Static(s) => s.to_json(),
      SourceMapCell::Owned(owned) => owned.to_json(),
    }
  }

  /// Generate source map to writer.
  pub fn to_writer<W: std::io::Write>(&self, w: W) -> Result<()> {
    match &self.0 {
      SourceMapCell::Static(s) => s.to_writer(w),
      SourceMapCell::Owned(owned) => owned.to_writer(w),
    }
  }
}
