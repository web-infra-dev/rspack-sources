use std::{borrow::Cow, fmt};

use crate::helpers::{
  create_mapping_serializer, MappingSerializer, StreamChunks,
};

pub type BoxSource = Box<dyn Source>;

pub trait Source: StreamChunks {
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
  file: Option<String>,
  mappings: Mappings,
  sources: Vec<Option<String>>,
  sources_content: Vec<Option<String>>,
  names: Vec<Option<String>>,
}

impl SourceMap {
  pub fn new<M: Into<Mappings>>(
    file: Option<String>,
    mappings: M,
    sources: Vec<Option<String>>,
    sources_content: Vec<Option<String>>,
    names: Vec<Option<String>>,
  ) -> Self {
    Self {
      file,
      mappings: mappings.into(),
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

  pub fn serialize(&self) -> Option<String> {
    let mut serializer = create_mapping_serializer(&self.options);
    self
      .inner
      .iter()
      .map(|mapping| mapping.serialize(&mut serializer))
      .collect()
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

impl Mapping {
  pub fn serialize<T: MappingSerializer>(
    &self,
    serializer: &mut T,
  ) -> Option<String> {
    serializer.serialize(self)
  }
}
