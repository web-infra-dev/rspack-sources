use sourcemap::{SourceMap, SourceMapBuilder, Token};

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

  original_source_ensured: bool,
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

      original_source_ensured: false,
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

      original_source_ensured: false,
    })
  }

  pub(crate) fn ensure_original_source(&mut self) {
    if !self.original_source_ensured {
      let current_file_name = self.name.as_str();
      let source_idx = self
        .source_map
        .sources()
        .enumerate()
        .find_map(|(idx, source)| {
          if source == current_file_name {
            Some(idx)
          } else {
            None
          }
        });

      if let Some(source_idx) = source_idx {
        if self.source_map.get_source(source_idx as u32).is_none() {
          self.source_map.set_source_contents(
            source_idx as u32,
            self.original_source.as_ref().map(|s| s.as_str()),
          );
        }
      }

      self.original_source_ensured = true;
    }
  }

  fn find_original_token_in_tuple<'a, 'b>(&'a self, token: &'b Token<'a>) -> Token<'a> {
    if let Some(inner_source_map) = &self.inner_source_map {
      let source = token.get_source();
      let src_line = token.get_src_line();
      let src_col = token.get_src_col();

      if matches!(inner_source_map.get_file(), Some(source)) {
        if let Some(original_token) = inner_source_map.lookup_token(src_line, src_col) {
          original_token
        } else {
          token.clone()
        }
      } else {
        token.clone()
      }
    } else {
      token.clone()
    }
  }

  pub(crate) fn remap_with_inner_sourcemap(&mut self) {
    let mut source_map_builder = SourceMapBuilder::new(Some(&self.name));

    if self.inner_source_map.is_some() {
      let source_map = &self.source_map;
      source_map.tokens().for_each(|token| {
        let original_token = self.find_original_token_in_tuple(&token);

        source_map_builder.add(
          original_token.get_dst_line(),
          original_token.get_dst_col(),
          original_token.get_src_line(),
          original_token.get_src_col(),
          original_token.get_source(),
          original_token.get_name(),
        );
      })
    }
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
