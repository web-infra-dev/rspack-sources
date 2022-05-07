use crate::{
  helpers::get_map,
  source::{GenMapOption, Source},
};

use sourcemap::SourceMap;

pub struct SourceMapSource {
  value: Vec<u8>,
  name: String,
  source_map: SourceMap,
  original_source: Option<Vec<u8>>,
  inner_source_map: Option<SourceMap>,
  remove_original_source: Option<bool>,
}

impl SourceMapSource {
  pub fn new(
    source_code: Vec<u8>,
    name: String,
    source_map: SourceMap,
    original_source: Option<Vec<u8>>,
    inner_source_map: Option<SourceMap>,
    remove_original_source: Option<bool>,
  ) -> SourceMapSource {
    SourceMapSource {
      value: source_code,
      name,
      source_map,
      original_source,
      inner_source_map,
      remove_original_source,
    }
  }
}

impl Source for SourceMapSource {
  fn source(&mut self) -> Vec<u8> {
    self.value.clone()
  }

  fn map(&mut self, option: GenMapOption) -> Option<SourceMap> {
    match self.inner_source_map {
      None => Some(self.source_map.clone()),
      Some(_) => get_map(option),
    }
  }
}
