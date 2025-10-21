use std::{
  any::{Any, TypeId},
  borrow::Cow,
  fmt::{self, Debug},
  hash::{Hash, Hasher},
  sync::Arc,
};

use serde::Serialize;
use simd_json::{
  base::ValueAsScalar,
  derived::{ValueObjectAccessAsArray, ValueObjectAccessAsScalar},
  BorrowedValue,
};

use crate::{
  helpers::{decode_mappings, StreamChunks},
  rope::Rope,
  Mapping, Result,
};

#[derive(Clone, Eq, Serialize)]
pub enum StringRef<'a> {
  Borrowed(&'a str),
  Shared(Arc<str>),
}

impl<'a> StringRef<'a> {
  pub fn as_str(&self) -> &str {
    match self {
      StringRef::Borrowed(s) => s,
      StringRef::Shared(s) => s.as_ref(),
    }
  }

  pub fn into_owned(self) -> StringRef<'static> {
    match self {
      StringRef::Borrowed(s) => StringRef::Shared(Arc::from(s)),
      StringRef::Shared(s) => StringRef::Shared(s),
    }
  }

  pub fn as_borrowed(&'a self) -> Self {
    match &self {
      StringRef::Borrowed(s) => StringRef::Borrowed(s),
      StringRef::Shared(s) => StringRef::Borrowed(s.as_ref()),
    }
  }
}

impl PartialEq for StringRef<'_> {
  fn eq(&self, other: &Self) -> bool {
    self.as_str() == other.as_str()
  }
}

impl<'a> From<&'a str> for StringRef<'a> {
  fn from(s: &'a str) -> Self {
    StringRef::Borrowed(s)
  }
}

impl From<String> for StringRef<'_> {
  fn from(s: String) -> Self {
    StringRef::Shared(Arc::from(s))
  }
}

impl From<Arc<str>> for StringRef<'_> {
  fn from(s: Arc<str>) -> Self {
    StringRef::Shared(s)
  }
}

impl Hash for StringRef<'_> {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.as_str().hash(state);
  }
}

impl AsRef<str> for StringRef<'_> {
  fn as_ref(&self) -> &str {
    self.as_str()
  }
}

fn is_all_empty(val: &[Cow<'_, str>]) -> bool {
  if val.is_empty() {
    return true;
  }
  val.iter().all(|s| s.is_empty())
}

#[derive(Clone, PartialEq, Eq, Serialize)]
struct BorrowedSourceMap<'a> {
  version: u8,
  #[serde(skip_serializing_if = "Option::is_none")]
  file: Option<Cow<'a, str>>,
  sources: Vec<Cow<'a, str>>,
  #[serde(rename = "sourcesContent", skip_serializing_if = "is_all_empty")]
  sources_content: Vec<Cow<'a, str>>,
  names: Vec<Cow<'a, str>>,
  mappings: Cow<'a, str>,
  #[serde(rename = "sourceRoot", skip_serializing_if = "Option::is_none")]
  source_root: Option<Cow<'a, str>>,
  #[serde(rename = "debugId", skip_serializing_if = "Option::is_none")]
  debug_id: Option<Cow<'a, str>>,
  #[serde(rename = "ignoreList", skip_serializing_if = "Option::is_none")]
  ignore_list: Option<Vec<u32>>,
}

impl Hash for BorrowedSourceMap<'_> {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.file.hash(state);
    self.mappings.hash(state);
    self.sources.hash(state);
    self.sources_content.hash(state);
    self.names.hash(state);
    self.source_root.hash(state);
    self.ignore_list.hash(state);
  }
}

impl BorrowedSourceMap<'_> {
  pub fn into_owned(self) -> BorrowedSourceMap<'static> {
    fn cow_to_owned(s: Cow<'_, str>) -> Cow<'static, str> {
      match s {
        Cow::Borrowed(s) => Cow::Owned(s.to_string()),
        Cow::Owned(s) => Cow::Owned(s),
      }
    }

    BorrowedSourceMap {
      version: self.version,
      file: self.file.map(cow_to_owned),
      sources: self
        .sources
        .into_iter()
        .map(cow_to_owned)
        .collect::<Vec<_>>()
        .into(),
      sources_content: self
        .sources_content
        .into_iter()
        .map(cow_to_owned)
        .collect::<Vec<_>>()
        .into(),
      names: self
        .names
        .into_iter()
        .map(cow_to_owned)
        .collect::<Vec<_>>()
        .into(),
      mappings: cow_to_owned(self.mappings),
      source_root: self.source_root.map(cow_to_owned),
      debug_id: self.debug_id.map(cow_to_owned),
      ignore_list: self.ignore_list,
    }
  }

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

impl Clone for StaticSourceMap {
  fn clone(&self) -> Self {
    Self::new(self.borrow_owner().clone(), |_| {
      let dependent = self.borrow_dependent();
      unsafe {
        std::mem::transmute::<BorrowedSourceMap, BorrowedSourceMap<'static>>(
          dependent.clone(),
        )
      }
    })
  }
}

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
            .map(|s| Cow::Borrowed(s.as_str().unwrap_or_default()))
            .collect::<Vec<_>>()
        })
        .unwrap_or_default()
        .into(),
      sources_content: owner
        .borrow_dependent()
        .get_array("sourcesContent")
        .map(|v| {
          v.iter()
            .map(|s| Cow::Borrowed(s.as_str().unwrap_or_default()))
            .collect::<Vec<_>>()
        })
        .unwrap_or_default()
        .into(),
      names: owner
        .borrow_dependent()
        .get_array("names")
        .map(|v| {
          v.iter()
            .map(|s| Cow::Borrowed(s.as_str().unwrap_or_default()))
            .collect::<Vec<_>>()
        })
        .unwrap_or_default()
        .into(),
      mappings: owner
        .borrow_dependent()
        .get_str("mappings")
        .unwrap_or_default()
        .into(),
      source_root: owner
        .borrow_dependent()
        .get_str("sourceRoot")
        .map(Into::into),
      debug_id: owner.borrow_dependent().get_str("debugId").map(Into::into),
      ignore_list: owner.borrow_dependent().get_array("ignoreList").map(|v| {
        v.iter()
          .map(|n| n.as_u32().unwrap_or_default())
          .collect::<Vec<_>>()
      }),
    })
  }

  pub fn from_json(json: String) -> Result<Self> {
    let borrowed_value_cell =
      BorrowedValueCell::try_new(json.into_bytes(), |owner| {
        // We need a mutable slice from our owned data
        // SAFETY: We're creating a mutable reference to the owned data.
        // The self_cell ensures this reference is valid for the lifetime of the cell.
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

#[derive(Clone, Eq, Hash)]
enum SourceMapCell<'a> {
  Static(StaticSourceMap),
  Borrowed(BorrowedSourceMap<'a>),
}

impl PartialEq for SourceMapCell<'_> {
  fn eq(&self, other: &Self) -> bool {
    match (self, other) {
      (SourceMapCell::Static(this), SourceMapCell::Static(other)) => {
        this.borrow_dependent() == other.borrow_dependent()
      }
      (SourceMapCell::Static(this), SourceMapCell::Borrowed(other)) => {
        this.borrow_dependent() == other
      }
      (SourceMapCell::Borrowed(this), SourceMapCell::Static(other)) => {
        this == other.borrow_dependent()
      }
      (SourceMapCell::Borrowed(this), SourceMapCell::Borrowed(other)) => {
        this == other
      }
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
pub struct SourceMap<'a>(SourceMapCell<'a>);

impl<'a> SourceMap<'a> {
  /// Create a [SourceMap].
  pub fn new<Mappings>(
    mappings: Mappings,
    sources: Vec<Cow<'a, str>>,
    sources_content: Vec<Cow<'a, str>>,
    names: Vec<Cow<'a, str>>,
  ) -> Self
  where
    Mappings: Into<Cow<'a, str>>,
  {
    Self(SourceMapCell::Borrowed(BorrowedSourceMap {
      version: 3,
      file: None,
      sources: sources.into(),
      sources_content: sources_content.into(),
      names: names.into(),
      mappings: mappings.into(),
      source_root: None,
      debug_id: None,
      ignore_list: None,
    }))
  }

  /// Get the file field in [SourceMap].
  pub fn file(&self) -> Option<&str> {
    match &self.0 {
      SourceMapCell::Static(owned) => {
        owned.borrow_dependent().file.as_ref().map(|s| s.as_ref())
      }
      SourceMapCell::Borrowed(borrowed) => {
        borrowed.file.as_ref().map(|s| s.as_ref())
      }
    }
  }

  /// Set the file field in [SourceMap].
  pub fn set_file<T: Into<String>>(&mut self, file: Option<T>) {
    match &mut self.0 {
      SourceMapCell::Static(owned) => {
        owned.with_dependent_mut(|_, dependent| {
          dependent.file = file.map(|s| s.into().into());
        })
      }
      SourceMapCell::Borrowed(borrowed) => {
        borrowed.file = file.map(|s| s.into().into())
      }
    }
  }

  /// Get the ignoreList field in [SourceMap].
  pub fn ignore_list(&self) -> Option<&[u32]> {
    match &self.0 {
      SourceMapCell::Static(owned) => {
        owned.borrow_dependent().ignore_list.as_deref()
      }
      SourceMapCell::Borrowed(borrowed) => borrowed.ignore_list.as_deref(),
    }
  }

  /// Set the ignoreList field in [SourceMap].
  pub fn set_ignore_list<T: Into<Vec<u32>>>(&mut self, ignore_list: Option<T>) {
    match &mut self.0 {
      SourceMapCell::Static(owned) => {
        owned.with_dependent_mut(|_, dependent| {
          dependent.ignore_list = ignore_list.map(|v| v.into());
        })
      }
      SourceMapCell::Borrowed(borrowed) => {
        borrowed.ignore_list = ignore_list.map(|v| v.into());
      }
    }
  }

  /// Get the decoded mappings in [SourceMap].
  pub fn decoded_mappings(&self) -> impl Iterator<Item = Mapping> + '_ {
    decode_mappings(self)
  }

  /// Get the mappings string in [SourceMap].
  pub fn mappings(&self) -> &str {
    match &self.0 {
      SourceMapCell::Static(owned) => {
        owned.borrow_dependent().mappings.as_ref()
      }
      SourceMapCell::Borrowed(borrowed) => borrowed.mappings.as_ref(),
    }
  }

  /// Get the sources field in [SourceMap].
  pub fn sources(&self) -> &[Cow<'_, str>] {
    match &self.0 {
      SourceMapCell::Static(owned) => &owned.borrow_dependent().sources,
      SourceMapCell::Borrowed(borrowed) => &borrowed.sources,
    }
  }

  /// Set the sources field in [SourceMap].
  pub fn set_sources(&mut self, sources: Vec<String>) {
    match &mut self.0 {
      SourceMapCell::Static(owned) => {
        owned.with_dependent_mut(|_, dependent| {
          dependent.sources = sources
            .into_iter()
            .map(Cow::Owned)
            .collect::<Vec<_>>()
            .into();
        })
      }
      SourceMapCell::Borrowed(borrowed) => {
        borrowed.sources = sources
          .into_iter()
          .map(Cow::Owned)
          .collect::<Vec<_>>()
          .into();
      }
    }
  }

  /// Get the source by index from sources field in [SourceMap].
  pub fn get_source(&self, index: usize) -> Option<&str> {
    match &self.0 {
      SourceMapCell::Static(owned) => owned
        .borrow_dependent()
        .sources
        .get(index)
        .map(AsRef::as_ref),
      SourceMapCell::Borrowed(borrowed) => {
        borrowed.sources.get(index).map(AsRef::as_ref)
      }
    }
  }

  /// Get the sourcesContent field in [SourceMap].
  pub fn sources_content(&self) -> &[Cow<'_, str>] {
    match &self.0 {
      SourceMapCell::Static(owned) => &owned.borrow_dependent().sources_content,
      SourceMapCell::Borrowed(borrowed) => &borrowed.sources_content,
    }
  }

  /// Set the sourcesContent field in [SourceMap].
  pub fn set_sources_content(&mut self, sources_content: Vec<String>) {
    match &mut self.0 {
      SourceMapCell::Static(owned) => {
        owned.with_dependent_mut(|_, dependent| {
          dependent.sources = sources_content
            .into_iter()
            .map(Cow::Owned)
            .collect::<Vec<_>>();
        })
      }
      SourceMapCell::Borrowed(borrowed) => {
        borrowed.sources = sources_content
          .into_iter()
          .map(Cow::Owned)
          .collect::<Vec<_>>();
      }
    }
  }

  /// Get the source content by index from sourcesContent field in [SourceMap].
  pub fn get_source_content(&self, index: usize) -> Option<&str> {
    match &self.0 {
      SourceMapCell::Static(owned) => owned
        .borrow_dependent()
        .sources_content
        .get(index)
        .map(AsRef::as_ref),
      SourceMapCell::Borrowed(borrowed) => {
        borrowed.sources_content.get(index).map(AsRef::as_ref)
      }
    }
  }

  /// Get the names field in [SourceMap].
  pub fn names(&self) -> &[Cow<'_, str>] {
    match &self.0 {
      SourceMapCell::Static(owned) => &owned.borrow_dependent().names,
      SourceMapCell::Borrowed(borrowed) => &borrowed.names,
    }
  }

  /// Set the names field in [SourceMap].
  pub fn set_names(&mut self, names: Vec<String>) {
    let names_vec: Vec<Cow<'static, str>> =
      names.into_iter().map(Cow::Owned).collect::<Vec<_>>();

    match &mut self.0 {
      SourceMapCell::Static(owned) => {
        owned.with_dependent_mut(|_, dependent| {
          dependent.names = names_vec;
        })
      }
      SourceMapCell::Borrowed(borrowed) => {
        borrowed.names = names_vec;
      }
    }
  }

  /// Get the name by index from names field in [SourceMap].
  pub fn get_name(&self, index: usize) -> Option<&str> {
    match &self.0 {
      SourceMapCell::Static(owned) => {
        owned.borrow_dependent().names.get(index).map(AsRef::as_ref)
      }
      SourceMapCell::Borrowed(borrowed) => {
        borrowed.names.get(index).map(AsRef::as_ref)
      }
    }
  }

  /// Get the source_root field in [SourceMap].
  pub fn source_root(&self) -> Option<&str> {
    match &self.0 {
      SourceMapCell::Static(owned) => owned
        .borrow_dependent()
        .source_root
        .as_ref()
        .map(|s| s.as_ref()),
      SourceMapCell::Borrowed(borrowed) => {
        borrowed.source_root.as_ref().map(|s| s.as_ref())
      }
    }
  }

  /// Set the source_root field in [SourceMap].
  pub fn set_source_root<T: Into<String>>(&mut self, source_root: Option<T>) {
    match &mut self.0 {
      SourceMapCell::Static(owned) => {
        owned.with_dependent_mut(|_, dependent| {
          dependent.source_root = source_root.map(|s| s.into().into());
        })
      }
      SourceMapCell::Borrowed(borrowed) => {
        borrowed.source_root = source_root.map(|s| s.into().into());
      }
    }
  }

  /// Set the debug_id field in [SourceMap].
  pub fn set_debug_id(&mut self, debug_id: Option<String>) {
    match &mut self.0 {
      SourceMapCell::Static(owned) => {
        owned.with_dependent_mut(|_, dependent| {
          dependent.debug_id = debug_id.map(Into::into);
        })
      }
      SourceMapCell::Borrowed(borrowed) => {
        borrowed.debug_id = debug_id.map(Into::into);
      }
    }
  }

  /// Get the debug_id field in [SourceMap].
  pub fn get_debug_id(&self) -> Option<&str> {
    match &self.0 {
      SourceMapCell::Static(owned) => owned
        .borrow_dependent()
        .debug_id
        .as_ref()
        .map(|s| s.as_ref()),
      SourceMapCell::Borrowed(borrowed) => {
        borrowed.debug_id.as_ref().map(|s| s.as_ref())
      }
    }
  }

  /// Converts this source map into a version with `'static` lifetime.
  pub fn into_owned(self) -> SourceMap<'static> {
    match self.0 {
      SourceMapCell::Static(owned) => SourceMap(SourceMapCell::Static(owned)),
      SourceMapCell::Borrowed(borrowed) => {
        SourceMap(SourceMapCell::Borrowed(borrowed.into_owned()))
      }
    }
  }

  /// Creates a borrowed representation of this source map with lifetime `'a`.
  pub fn as_borrowed(&'a self) -> Self {
    match &self.0 {
      SourceMapCell::Static(owned) => {
        Self(SourceMapCell::Borrowed(BorrowedSourceMap {
          version: owned.borrow_dependent().version,
          file: owned
            .borrow_dependent()
            .file
            .as_ref()
            .map(|s| Cow::Borrowed(s.as_ref())),
          sources: owned.borrow_dependent().sources.clone(),
          sources_content: owned.borrow_dependent().sources_content.clone(),
          names: owned.borrow_dependent().names.clone(),
          mappings: owned.borrow_dependent().mappings.clone(),
          source_root: owned.borrow_dependent().source_root.clone(),
          debug_id: owned.borrow_dependent().debug_id.clone(),
          ignore_list: owned.borrow_dependent().ignore_list.clone(),
        }))
      }
      SourceMapCell::Borrowed(borrowed) => {
        Self(SourceMapCell::Borrowed(borrowed.clone()))
      }
    }
  }
}

impl std::fmt::Debug for SourceMap<'_> {
  fn fmt(
    &self,
    f: &mut std::fmt::Formatter<'_>,
  ) -> std::result::Result<(), std::fmt::Error> {
    let indent = f.width().unwrap_or(0);
    let indent_str = format!("{:indent$}", "", indent = indent);

    write!(
      f,
      "{indent_str}SourceMap::from_json({:?}.to_string()).unwrap()",
      self.clone().to_json().unwrap()
    )?;

    Ok(())
  }
}

impl SourceMap<'_> {
  /// Create a [SourceMap] from json string.
  pub fn from_json(json: impl Into<String>) -> Result<SourceMap<'static>> {
    let owned = StaticSourceMap::from_json(json.into())?;
    Ok(SourceMap(SourceMapCell::Static(owned)))
  }

  /// Create a [SourceMap] from reader.
  pub fn from_reader<R: std::io::Read>(mut s: R) -> Result<Self> {
    let mut json = String::default();
    s.read_to_string(&mut json)?;
    Self::from_json(json)
  }

  /// Generate source map to a json string.
  pub fn to_json(&self) -> Result<String> {
    match &self.0 {
      SourceMapCell::Static(owned) => owned.to_json(),
      SourceMapCell::Borrowed(borrowed) => borrowed.to_json(),
    }
  }

  /// Generate source map to writer.
  pub fn to_writer<W: std::io::Write>(&self, w: W) -> Result<()> {
    match &self.0 {
      SourceMapCell::Static(owned) => owned.to_writer(w),
      SourceMapCell::Borrowed(borrowed) => borrowed.to_writer(w),
    }
  }
}
