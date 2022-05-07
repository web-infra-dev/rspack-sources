use sourcemap::SourceMap;

use crate::result::Error;
use crate::{
  helpers::get_map,
  source::{GenMapOption, Source},
};

pub struct SourceMapSource {
  pub(crate) source_code: String,
  pub(crate) name: String,
  pub(crate) source_map: SourceMap,
  pub(crate) original_source: Option<String>,
  pub(crate) inner_source_map: Option<SourceMap>,
  pub(crate) remove_original_source: Option<bool>,
}

pub struct SourceMapSourceUtf8Options {
  pub source_code: Vec<u8>,
  pub name: String,
  pub source_map: SourceMap,
  pub original_source: Option<Vec<u8>>,
  pub inner_source_map: Option<SourceMap>,
  pub remove_original_source: Option<bool>,
}

impl SourceMapSource {
  pub fn new(
    source_code: String,
    name: String,
    source_map: SourceMap,
    original_source: Option<String>,
    inner_source_map: Option<SourceMap>,
    remove_original_source: Option<bool>,
  ) -> Self {
    Self {
      source_code,
      name,
      source_map,
      original_source,
      inner_source_map,
      remove_original_source,
    }
  }

  pub fn from_utf8(options: SourceMapSourceUtf8Options) -> Result<Self, Error> {
    let SourceMapSourceUtf8Options {
      source_code,
      name,
      source_map,
      original_source,
      inner_source_map,
      remove_original_source,
    } = options;

    let original_source = if let Some(original_source) = original_source {
      Some(String::from_utf8(original_source)?)
    } else {
      None
    };

    Ok(Self {
      source_code: String::from_utf8(source_code)?,
      name,
      source_map,
      original_source,
      inner_source_map,
      remove_original_source,
    })
  }
}

impl Source for SourceMapSource {
  fn source(&self) -> String {
    self.source_code.clone()
  }

  fn map(&mut self, option: GenMapOption) -> Option<SourceMap> {
    match self.inner_source_map {
      None => Some(self.source_map.clone()),
      Some(_) => get_map(option),
    }
  }
}
