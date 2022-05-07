use std::ops::Add;

use sourcemap::SourceMap;

use crate::{
  helpers::get_map,
  source::{GenMapOption, Source},
  source_map_source::SourceMapSource,
};

pub enum ConcattableSource {
  SourceMapSource(SourceMapSource),
  // TODO:
  // ConcatSource(ConcatSource),
  // CachedSource
}

pub struct ConcatSource {
  children: Vec<ConcattableSource>,
}

impl ConcatSource {
  pub fn new(items: Vec<ConcattableSource>) -> Self {
    Self { children: items }
  }

  pub fn add(&mut self, item: ConcattableSource) {
    self.children.push(item);
  }

  pub(crate) fn concat_each_impl(
    prev_map: Option<SourceMap>,
    concattable: &mut ConcattableSource,
  ) -> SourceMap {
    match concattable {
      ConcattableSource::SourceMapSource(s) => {
        let current_file_name = s.name.as_str();
        let source_idx = s
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
          if s.source_map.get_source(source_idx as u32).is_none() {
            s.source_map
              .set_source_contents(source_idx as u32, s.original_source.map(|s| s.as_str()));
          }
        }

        todo!()
      }
    }
  }
}

impl Source for ConcatSource {
  fn source(&self) -> String {
    let mut code = "".to_owned();
    self.children.iter().for_each(|child| {
      let source = match child {
        ConcattableSource::SourceMapSource(s) => s.source(),
        // ConcattableSource::ConcatSource(s) => s.source(),
      };
      code += &source;
    });
    code
  }

  fn map(&mut self, option: GenMapOption) -> Option<SourceMap> {
    let mut prev_map: Option<SourceMap> = None;
    self.children.iter_mut().for_each(|concattable| {
      let new_map = ConcatSource::concat_each_impl(prev_map, concattable);
      prev_map = Some(new_map);
    });

    // TODO:
    // get_map(option)
    prev_map
  }
}
