use std::{borrow::Cow, fmt, ops};

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
  fn map(&self, options: MapOptions) -> Option<SourceMap>;
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
  sources: Vec<Option<String>>,
  sources_content: Vec<Option<String>>,
  names: Vec<Option<String>>,
}

impl SourceMap {
  pub fn new(
    file: Option<String>,
    mappings: Mappings,
    source_root: Option<String>,
    sources: Vec<Option<String>>,
    sources_content: Vec<Option<String>>,
    names: Vec<Option<String>>,
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

  pub fn sources(&self) -> impl Iterator<Item = Option<&str>> {
    self.sources.iter().map(|s| s.as_deref())
  }

  pub fn get_source(&self, index: usize) -> Option<String> {
    let source = self.sources.get(index);
    source.and_then(|source| source.as_deref()).map(|source| {
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

  pub fn sources_content(&self) -> impl Iterator<Item = Option<&str>> {
    self.sources_content.iter().map(|s| s.as_deref())
  }

  pub fn names(&self) -> impl Iterator<Item = Option<&str>> {
    self.names.iter().map(|s| s.as_deref())
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mappings {
  options: MapOptions,
  inner: Vec<Mapping>,
}

impl ops::Deref for Mappings {
  type Target = Vec<Mapping>;

  fn deref(&self) -> &Self::Target {
    &self.inner
  }
}

impl ops::DerefMut for Mappings {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.inner
  }
}

impl Mappings {
  pub fn new<M, T, O>(inner: T, options: O) -> Self
  where
    M: Into<Mapping>,
    T: IntoIterator<Item = M>,
    O: Into<MapOptions>,
  {
    Self {
      options: options.into(),
      inner: inner.into_iter().map(|m| m.into()).collect(),
    }
  }

  pub fn serialize(&self) -> String {
    create_mappings_serializer(&self.options).serialize(&self.inner)
  }

  pub fn deserialize<O>(mappings: &str, options: O) -> Result<Self>
  where
    O: Into<MapOptions>,
  {
    let inner = NormalMappingsDeserializer::default().deserialize(mappings)?;
    Ok(Self {
      options: options.into(),
      inner,
    })
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
  ($options:expr, $mappings:literal $(,)?) => {
    $crate::Mappings::deserialize($mappings, $options).unwrap()
  };

  ($options:expr, $($mapping:expr),* $(,)?) => {
    $crate::Mappings::new(::std::vec![$({
      let mapping = $mapping;
      $crate::m![mapping[0], mapping[1], mapping[2], mapping[3], mapping[4], mapping[5]]
    }),*], $options)
  };
}
