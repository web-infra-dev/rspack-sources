use rayon::prelude::*;
use smol_str::SmolStr;
use sourcemap::{RawToken, SourceMap, SourceMapBuilder, Token};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::{
  source::{GenMapOption, Source},
  Error,
};

pub struct ConcatSource<'a> {
  children: Vec<&'a mut (dyn Source + Send)>,
}

impl<'a> ConcatSource<'a> {
  pub fn new(items: Vec<&'a mut (dyn Source + Send)>) -> Self {
    Self { children: items }
  }

  pub fn add(&mut self, item: &'a mut (dyn Source + Send)) {
    self.children.push(item);
  }

  pub fn generate_string(
    &mut self,
    gen_map_options: &GenMapOption,
  ) -> Result<Option<String>, Error> {
    let source_map = self.map(gen_map_options);
    let is_source_map_exist = source_map.is_some();

    let mut writer: Vec<u8> = Default::default();
    source_map.map(|sm| sm.to_writer(&mut writer));

    Ok(if is_source_map_exist {
      Some(String::from_utf8(writer)?)
    } else {
      None
    })
  }

  pub fn generate_base64(
    &mut self,
    gen_map_options: &GenMapOption,
  ) -> Result<Option<String>, Error> {
    let map_string = self.generate_string(gen_map_options)?;
    Ok(map_string.map(base64::encode))
  }

  pub fn generate_url(&mut self, gen_map_options: &GenMapOption) -> Result<Option<String>, Error> {
    let map_base64 = self.generate_base64(gen_map_options)?;

    Ok(map_base64.map(|s| format!("data:application/json;charset=utf-8;base64,{}", s)))
  }
}

impl<'a> Source for ConcatSource<'a> {
  fn map(&mut self, option: &GenMapOption) -> Option<SourceMap> {
    let source_contents_map: Arc<Mutex<HashMap<String, Option<String>>>> =
      Arc::new(Mutex::new(HashMap::default()));
    let children_sourcemap = self
      .children
      .par_iter_mut()
      .map(|concattable| {
        let source_map = concattable.map(option);
        if let Some(source_map) = source_map {
          return source_map
            .tokens()
            .map(|token| {
              let source = token.get_source().map(|s| s.to_owned());
              let name = token.get_name().map(|s| s.to_owned());
              if let Some(source) = source.as_ref() {
                source_contents_map
                  .lock()
                  .unwrap()
                  .entry(source.clone())
                  .or_insert_with(|| {
                    source_map
                      .get_source_contents(token.get_src_id())
                      .map(|s| s.to_owned())
                  });
              }

              let RawToken {
                dst_col,
                dst_line,
                src_line,
                src_col,
                ..
              } = token.get_raw_token();
              (dst_line, dst_col, src_line, src_col, source, name)
            })
            .collect::<Vec<_>>();
        }
        Default::default()
      })
      .collect::<Vec<Vec<(u32, u32, u32, u32, Option<String>, Option<String>)>>>();

    let mut cur_gen_line = 0u32;
    let mut source_map_builder = SourceMapBuilder::new(option.file.as_deref());

    children_sourcemap
      .into_iter()
      .zip(self.children.iter_mut())
      .for_each(|(tokens, concattable)| {
        let mut prev_line = 0u32;
        let mut starting_line = cur_gen_line;

        tokens
          .into_iter()
          .for_each(|(dst_line, dst_col, src_line, src_col, source, name)| {
            let line_diff = dst_line - prev_line;
            let raw_token = source_map_builder.add(
              starting_line + line_diff,
              dst_col,
              src_line,
              src_col,
              source.as_deref(),
              name.as_deref(),
            );

            if option.include_source_contents {
              if let Some(source) = source.as_ref() {
                if let Some(sm) = source_contents_map.lock().unwrap().get(source) {
                  source_map_builder.set_source_contents(raw_token.src_id, sm.as_deref())
                }
              }
            }

            starting_line += line_diff;
            prev_line = dst_line;
          });

        // why not `lines`? `lines` will trim the trailing `\n`, which generates the wrong sourcemap
        let line_len = concattable.source().split('\n').count();
        cur_gen_line += line_len as u32;
      });

    Some(source_map_builder.into_sourcemap())
  }

  fn source(&mut self) -> SmolStr {
    self
      .children
      .iter_mut()
      .map(|child| child.source())
      .collect::<Vec<_>>()
      .join("\n")
      .into()
  }
}