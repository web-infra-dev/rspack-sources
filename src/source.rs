use std::{borrow::Cow, fmt, ops};

use serde::{Deserialize, Serialize};

use crate::{
  helpers::{
    create_mappings_serializer, MappingsDeserializer, MappingsSerializer,
    NormalMappingsDeserializer, StreamChunks,
  },
  Result,
};

pub type BoxSource = Box<dyn Source>;

pub trait Source: StreamChunks + fmt::Debug + Sync + Send {
  fn source(&self) -> Cow<str>;
  fn buffer(&self) -> Cow<[u8]>;
  fn size(&self) -> usize;
  fn map(&self, options: &MapOptions) -> Option<SourceMap>;
}

impl<T: Source + 'static> From<T> for BoxSource {
  fn from(s: T) -> Self {
    Box::new(s)
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMap {
  version: u8,
  file: Option<String>,
  mappings: Mappings,
  source_root: Option<String>,
  sources: Option<Vec<Option<String>>>,
  sources_content: Option<Vec<Option<String>>>,
  names: Option<Vec<Option<String>>>,
}

impl SourceMap {
  pub fn new(
    file: Option<String>,
    mappings: Mappings,
    source_root: Option<String>,
    sources: Option<Vec<Option<String>>>,
    sources_content: Option<Vec<Option<String>>>,
    names: Option<Vec<Option<String>>>,
  ) -> Self {
    Self {
      version: 3,
      file,
      mappings,
      source_root,
      sources,
      sources_content,
      names,
    }
  }

  pub fn file(&self) -> Option<&str> {
    self.file.as_deref()
  }

  pub fn set_file(&mut self, file: Option<String>) {
    self.file = file;
  }

  pub fn mappings(&self) -> &Mappings {
    &self.mappings
  }

  pub fn sources(&self) -> Option<&[Option<String>]> {
    self.sources.as_deref()
  }

  pub fn get_source(&self, index: usize) -> Option<String> {
    self
      .sources
      .as_ref()
      .and_then(|sources| sources.get(index))
      .and_then(|source| source.as_deref())
      .map(|source| {
        self
          .source_root
          .as_ref()
          .map(|source_root| {
            if source_root.ends_with('/') {
              format!("{source_root}{source}")
            } else {
              format!("{source_root}/{source}")
            }
          })
          .unwrap_or(source.to_owned())
      })
  }

  pub fn sources_content(&self) -> Option<&[Option<String>]> {
    self.sources_content.as_deref()
  }

  pub fn get_source_content(&self, index: usize) -> Option<&str> {
    self
      .sources_content
      .as_ref()
      .and_then(|sources_content| sources_content.get(index))
      .and_then(|source_content| source_content.as_deref())
  }

  pub fn names(&self) -> Option<&[Option<String>]> {
    self.names.as_deref()
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mappings(Vec<Mapping>);

impl ops::Deref for Mappings {
  type Target = Vec<Mapping>;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

impl ops::DerefMut for Mappings {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.0
  }
}

impl Mappings {
  pub fn new<M, T>(inner: T) -> Self
  where
    M: Into<Mapping>,
    T: IntoIterator<Item = M>,
  {
    Self(inner.into_iter().map(|m| m.into()).collect())
  }

  pub fn serialize(&self, options: &MapOptions) -> String {
    create_mappings_serializer(options).serialize(&self.0)
  }

  pub fn deserialize(mappings: &str) -> Result<Self> {
    let inner = NormalMappingsDeserializer::default().deserialize(mappings)?;
    Ok(Self(inner))
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mapping {
  pub generated_line: u32,
  pub generated_column: u32,
  pub original: Option<OriginalLocation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
  ($mappings:expr $(,)?) => {
    $crate::Mappings::deserialize($mappings).unwrap()
  };

  ($($mapping:expr),* $(,)?) => {
    $crate::Mappings::new(::std::vec![$({
      let mapping = $mapping;
      $crate::m![mapping[0], mapping[1], mapping[2], mapping[3], mapping[4], mapping[5]]
    }),*])
  };
}
