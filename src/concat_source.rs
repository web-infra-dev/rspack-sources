use std::{borrow::Cow, cell::RefCell, collections::HashMap};

use crate::{
  helpers::{get_map, GeneratedInfo, OnChunk, OnName, OnSource, StreamChunks},
  source::{Mapping, OriginalLocation},
  BoxSource, MapOptions, Source, SourceMap,
};

#[derive(Debug)]
pub struct ConcatSource {
  children: Vec<BoxSource>,
}

impl ConcatSource {
  pub fn new<S, T>(sources: S) -> Self
  where
    T: Into<BoxSource>,
    S: IntoIterator<Item = T>,
  {
    Self {
      children: sources.into_iter().map(|s| s.into()).collect(),
    }
  }

  pub fn add<S: Into<BoxSource>>(&mut self, item: S) {
    self.children.push(item.into());
  }
}

impl Source for ConcatSource {
  fn source(&self) -> Cow<str> {
    let all = self.children.iter().fold(String::new(), |mut acc, cur| {
      acc.push_str(&cur.source());
      acc
    });
    Cow::Owned(all)
  }

  fn buffer(&self) -> Cow<[u8]> {
    let all = self.children.iter().fold(Vec::new(), |mut acc, cur| {
      acc.extend(&*cur.buffer());
      acc
    });
    Cow::Owned(all)
  }

  fn size(&self) -> usize {
    self.children.iter().fold(0, |mut acc, cur| {
      acc += cur.size();
      acc
    })
  }

  fn map(&self, options: MapOptions) -> Option<SourceMap> {
    Some(get_map(self, options))
  }
}

impl StreamChunks for ConcatSource {
  fn stream_chunks(
    &self,
    options: &MapOptions,
    on_chunk: OnChunk,
    on_source: OnSource,
    on_name: OnName,
  ) -> crate::helpers::GeneratedInfo {
    if self.children.len() == 1 {
      return self.children[0]
        .stream_chunks(options, on_chunk, on_source, on_name);
    }
    let mut current_line_offset = 0;
    let mut current_column_offset = 0;
    let mut source_mapping: HashMap<String, u32> = HashMap::new();
    let mut name_mapping: HashMap<String, u32> = HashMap::new();
    let mut need_to_cloas_mapping = false;
    for item in &self.children {
      let source_index_mapping: RefCell<HashMap<u32, u32>> =
        RefCell::new(HashMap::new());
      let name_index_mapping: RefCell<HashMap<u32, u32>> =
        RefCell::new(HashMap::new());
      let mut last_mapping_line = 0;
      let GeneratedInfo {
        generated_line,
        generated_column,
      } = item.stream_chunks(
        options,
        &mut |mapping| {
          let line = mapping.generated_line + current_line_offset;
          let column = if mapping.generated_line == 1 {
            mapping.generated_column + current_column_offset
          } else {
            mapping.generated_column
          };
          if need_to_cloas_mapping {
            if mapping.generated_line != 1 || mapping.generated_column != 0 {
              on_chunk(Mapping {
                generated_line: current_line_offset + 1,
                generated_column: current_column_offset,
                original: None,
              });
            }
            need_to_cloas_mapping = false;
          }
          let result_source_index = mapping.original.as_ref().and_then(|original| {
            source_index_mapping
              .borrow()
              .get(&original.source_index)
              .copied()
          });
          let result_name_index = mapping
            .original.as_ref()
            .and_then(|original| original.name_index)
            .and_then(|name_index| {
              name_index_mapping.borrow().get(&name_index).copied()
            });
          last_mapping_line = if result_source_index.is_none() {
            0
          } else {
            mapping.generated_line
          };
          if options.final_source {
            if let Some(result_source_index) = result_source_index && let Some(original) = &mapping.original {
              on_chunk(Mapping {
                generated_line: line,
                generated_column: column,
                original: Some(OriginalLocation {
                  source_index: result_source_index,
                  original_line: original.original_line,
                  original_column: original.original_column,
                  name_index: result_name_index,
                }),
              });
            }
          } else {
            if let Some(result_source_index) = result_source_index && let Some(original) = &mapping.original {
              on_chunk(Mapping {
                generated_line: line,
                generated_column: column,
                original: Some(OriginalLocation {
                    source_index: result_source_index,
                    original_line: original.original_line,
                    original_column: original.original_column,
                    name_index: result_name_index,
                }),
              });
            } else {
              on_chunk(Mapping {
                generated_line: line,
                generated_column: column,
                original: None,
              });
            }
          }
        },
        &mut |i, source, source_content| {
          if let Some(source) = source {
            let mut global_index = source_mapping.get(source).copied();
            if global_index.is_none() {
              let len = source_mapping.len() as u32;
              source_mapping.insert(source.to_owned(), len);
              on_source(len, Some(source), source_content);
              global_index = Some(len);
            }
            source_index_mapping
              .borrow_mut()
              .insert(i, global_index.unwrap());
          }
        },
        &mut |i, name| {
          if let Some(name) = name {
            let mut global_index = name_mapping.get(name).copied();
            if global_index.is_none() {
              let len = name_mapping.len() as u32;
              name_mapping.insert(name.to_owned(), len);
              on_name(len, Some(name));
              global_index = Some(len);
            }
            name_index_mapping
              .borrow_mut()
              .insert(i, global_index.unwrap());
          }
        },
      );
      if need_to_cloas_mapping {
        if generated_line != 1 || generated_column != 0 {
          on_chunk(Mapping {
            generated_line: current_line_offset + 1,
            generated_column: current_column_offset,
            original: None,
          });
          need_to_cloas_mapping = false;
        }
      }
      if generated_line > 1 {
        current_column_offset = generated_column;
      } else {
        current_column_offset += generated_column;
      }
      need_to_cloas_mapping = need_to_cloas_mapping
        || (options.final_source && last_mapping_line == generated_line);
      current_line_offset += generated_line - 1;
    }
    GeneratedInfo {
      generated_line: current_line_offset + 1,
      generated_column: current_column_offset,
    }
  }
}

#[cfg(test)]
mod tests {
  use crate::{OriginalSource, RawSource};

  use super::*;

  #[test]
  fn should_concat_two_sources() {
    let mut source = ConcatSource::new([
      Box::new(RawSource::from("Hello World\n".to_string())) as BoxSource,
      Box::new(OriginalSource::new(
        "console.log('test');\nconsole.log('test2');\n",
        "console.js",
      )),
    ]);
    source.add(OriginalSource::new("Hello2\n", "hello.md"));

    let expected_source =
      "Hello World\nconsole.log('test');\nconsole.log('test2');\nHello2\n";
    assert_eq!(source.size(), 62);
    assert_eq!(source.source(), expected_source);

    // let expected_map1 = SourceMap::new(
    //   None,
    //   ";AAAA;AACA;ACDA".to_string(),
    //   vec![Some("console.js".to_string()), Some("hello.md".to_string())],
    //   vec![
    //     Some("console.log('test');\nconsole.log('test2');\n".to_string()),
    //     Some("Hello2\n".to_string()),
    //   ],
    //   vec![],
    // );
    // assert_eq!(source.map(MapOptions::new(false)).unwrap(), expected_map1);
    let map1 = source.map(MapOptions::new(false)).unwrap();
    assert_eq!(map1.mappings().serialize(), ";AAAA;AACA;ACDA");
    assert_eq!(map1.file(), None);
    assert_eq!(
      map1.sources().collect::<Vec<_>>(),
      [Some("console.js"), Some("hello.md")],
    );
    assert_eq!(
      map1.sources_content().collect::<Vec<_>>(),
      [
        Some("console.log('test');\nconsole.log('test2');\n"),
        Some("Hello2\n"),
      ],
    );
    assert_eq!(map1.names().collect::<Vec<_>>(), &[]);

    // let expected_map2 = SourceMap::new(
    //   None,
    //   ";AAAA;AACA;ACDA".to_string(),
    //   vec![Some("console.js".to_string()), Some("hello.md".to_string())],
    //   vec![
    //     Some("console.log('test');\nconsole.log('test2');\n".to_string()),
    //     Some("Hello2\n".to_string()),
    //   ],
    //   vec![],
    // );
    // assert_eq!(source.map(MapOptions::default()).unwrap(), expected_map2);
    let map2 = source.map(MapOptions::new(false)).unwrap();
    assert_eq!(map2.mappings().serialize(), ";AAAA;AACA;ACDA");
    assert_eq!(map2.file(), None);
    assert_eq!(
      map2.sources().collect::<Vec<_>>(),
      [Some("console.js"), Some("hello.md")],
    );
    assert_eq!(
      map2.sources_content().collect::<Vec<_>>(),
      [
        Some("console.log('test');\nconsole.log('test2');\n"),
        Some("Hello2\n"),
      ],
    );
    assert_eq!(map2.names().collect::<Vec<_>>(), &[]);
  }

  #[test]
  fn should_be_able_to_handle_strings_for_all_methods() {
    let mut source = ConcatSource::new([
      Box::new(RawSource::from("Hello World\n".to_string())) as BoxSource,
      Box::new(OriginalSource::new(
        "console.log('test');\nconsole.log('test2');\n",
        "console.js",
      )),
    ]);
    let inner_source =
      ConcatSource::new([RawSource::from("("), "'string'".into(), ")".into()]);
    source.add(RawSource::from("console"));
    source.add(RawSource::from("."));
    source.add(RawSource::from("log"));
    source.add(inner_source);
    let expected_source =
      "Hello World\nconsole.log('test');\nconsole.log('test2');\nconsole.log('string')";
    // let expected_map1 = SourceMap::new(
    //   None,
    //   ";AAAA;AACA".to_string(),
    //   vec![Some("console.js".to_string())],
    //   vec![Some(
    //     "console.log('test');\nconsole.log('test2');\n".to_string(),
    //   )],
    //   vec![],
    // );
    assert_eq!(source.size(), 76);
    assert_eq!(source.source(), expected_source);
    assert_eq!(source.buffer(), expected_source.as_bytes());

    let map = source.map(MapOptions::new(false)).unwrap();
    assert_eq!(map.mappings().serialize(), ";AAAA;AACA");
    assert!(map.file().is_none());
    assert_eq!(map.sources().collect::<Vec<_>>(), [Some("console.js")]);
    assert_eq!(
      map.sources_content().collect::<Vec<_>>(),
      [Some("console.log('test');\nconsole.log('test2');\n")],
    );
    assert_eq!(map.names().collect::<Vec<_>>(), []);

    // TODO: test hash
  }

  #[test]
  fn should_return_null_as_map_when_only_generated_code_is_concatenated() {
    let source = ConcatSource::new([
      RawSource::from("Hello World\n"),
      RawSource::from("Hello World\n".to_string()),
      RawSource::from(""),
    ]);

    let result_text = source.source();
    let result_map = source.map(MapOptions::default());
    let result_list_map = source.map(MapOptions::new(false));

    assert_eq!(result_text, "Hello World\nHello World\n");
    // assert!(result_map.is_none());
    // assert!(result_list_map.is_none());
  }

  #[test]
  fn should_allow_to_concatenate_in_a_single_line() {
    let source = ConcatSource::new([
      Box::new(OriginalSource::new("Hello", "hello.txt")) as BoxSource,
      Box::new(RawSource::from(" ")),
      Box::new(OriginalSource::new("World ", "world.txt")),
      Box::new(RawSource::from("is here\n")),
      Box::new(OriginalSource::new("Hello\n", "hello.txt")),
      Box::new(RawSource::from(" \n")),
      Box::new(OriginalSource::new("World\n", "world.txt")),
      Box::new(RawSource::from("is here")),
    ]);

    let map = source.map(MapOptions::default()).unwrap();
    assert_eq!(map.mappings().serialize(), "AAAA,K,CCAA,M;ADAA;;ACAA");
    assert!(map.file().is_none());
    assert_eq!(
      map.sources().collect::<Vec<_>>(),
      [Some("hello.txt"), Some("world.txt")],
    );
    assert_eq!(
      map.sources_content().collect::<Vec<_>>(),
      [Some("Hello"), Some("World ")],
    );
    assert_eq!(map.names().collect::<Vec<_>>(), []);
    // assert_eq!(
    //   source.map(MapOptions::default()).unwrap(),
    //   SourceMap::new(
    //     None,
    //     "AAAA,K,CCAA,M;ADAA;;ACAA".to_string(),
    //     vec![Some("hello.txt".to_string()), Some("world.txt".to_string())],
    //     vec![Some("Hello".to_string()), Some("World ".to_string())],
    //     vec![],
    //   ),
    // );
    assert_eq!(
      source.source(),
      "Hello World is here\nHello\n \nWorld\nis here",
    );
  }

  #[test]
  fn should_allow_to_concat_buffer_sources() {
    let source = ConcatSource::new([
      RawSource::from("a"),
      RawSource::from(Vec::from("b")),
      RawSource::from("c"),
    ]);
    assert_eq!(source.source(), "abc");
    // TODO
    // assert!(source.map(MapOptions::default()).is_none());
  }
}
