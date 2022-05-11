use smol_str::SmolStr;
use sourcemap::{SourceMap, SourceMapBuilder};

use crate::{
  source::{GenMapOption, Source},
  Error,
};

pub struct ConcatSource<'a> {
  children: Vec<&'a mut dyn Source>,
}

impl<'a> ConcatSource<'a> {
  pub fn new(items: Vec<&'a mut dyn Source>) -> Self {
    Self { children: items }
  }

  pub fn add(&mut self, item: &'a mut dyn Source) {
    self.children.push(item);
  }

  pub(crate) fn concat_each_impl(
    sm_builder: &mut SourceMapBuilder,
    mut cur_gen_line: u32,
    concattable: &'a mut dyn Source,
    gen_map_option: &GenMapOption,
  ) {
    let source_map = concattable.map(gen_map_option);

    let mut prev_line = 0u32;

    if let Some(source_map) = source_map.as_ref() {
      source_map.tokens().for_each(|token| {
        let line_diff = token.get_dst_line() - prev_line;

        let raw_token = sm_builder.add(
          cur_gen_line + line_diff,
          token.get_dst_col(),
          token.get_src_line(),
          token.get_src_col(),
          token.get_source(),
          token.get_name(),
        );

        if gen_map_option.include_source_contents {
          sm_builder.set_source_contents(
            raw_token.src_id,
            source_map.get_source_contents(token.get_src_id()),
          );
        }

        cur_gen_line += line_diff;

        prev_line = token.get_dst_line();
      });
    }
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

    Ok(map_base64.map(|mut s| {
      s = format!("data:application/json;charset=utf-8;base64,{}", s);
      s
    }))
  }
}

impl<'a> Source for ConcatSource<'a> {
  fn source(&mut self) -> SmolStr {
    self
      .children
      .iter_mut()
      .map(|child| child.source())
      .collect::<Vec<_>>()
      .join("\n")
      .into()
  }

  fn map(&mut self, option: &GenMapOption) -> Option<SourceMap> {
    let mut source_map_builder = SourceMapBuilder::new(option.file.as_deref());
    let mut cur_gen_line = 0u32;

    self.children.iter_mut().for_each(|concattable| {
      // why not `lines`? `lines` will trim the trailing `\n`, which generates the wrong sourcemap
      let line_len = concattable.source().split('\n').count();
      ConcatSource::concat_each_impl(&mut source_map_builder, cur_gen_line, *concattable, option);

      cur_gen_line += line_len as u32;
    });

    Some(source_map_builder.into_sourcemap())
  }
}
