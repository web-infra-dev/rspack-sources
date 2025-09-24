use std::{
  borrow::Cow,
  cell::RefCell,
  hash::{Hash, Hasher},
};

use rustc_hash::FxHashMap as HashMap;

use crate::{
  helpers::{get_map, GeneratedInfo, OnChunk, OnName, OnSource, StreamChunks},
  linear_map::LinearMap,
  source::{Mapping, OriginalLocation},
  BoxSource, MapOptions, Rope, Source, SourceExt, SourceMap,
};

/// Concatenate multiple [Source]s to a single [Source].
///
/// - [webpack-sources docs](https://github.com/webpack/webpack-sources/#concatsource).
///
/// ```
/// use rspack_sources::{
///   BoxSource, ConcatSource, MapOptions, OriginalSource, RawSource, Source,
///   SourceExt, SourceMap,
/// };
///
/// let mut source = ConcatSource::new([
///   RawSource::from("Hello World\n".to_string()).boxed(),
///   OriginalSource::new(
///     "console.log('test');\nconsole.log('test2');\n",
///     "console.js",
///   )
///   .boxed(),
/// ]);
/// source.add(OriginalSource::new("Hello2\n", "hello.md"));
///
/// assert_eq!(source.size(), 62);
/// assert_eq!(
///   source.source(),
///   "Hello World\nconsole.log('test');\nconsole.log('test2');\nHello2\n"
/// );
/// assert_eq!(
///   source.map(&MapOptions::new(false)).unwrap(),
///   SourceMap::from_json(
///     r#"{
///       "version": 3,
///       "mappings": ";AAAA;AACA;ACDA",
///       "names": [],
///       "sources": ["console.js", "hello.md"],
///       "sourcesContent": [
///         "console.log('test');\nconsole.log('test2');\n",
///         "Hello2\n"
///       ]
///     }"#,
///   )
///   .unwrap()
/// );
/// ```
#[derive(Default, Clone)]
pub struct ConcatSource {
  children: Vec<BoxSource>,
}

impl std::fmt::Debug for ConcatSource {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let indent = f.width().unwrap_or(0);
    let indent_str = format!("{:indent$}", "", indent = indent);

    writeln!(f, "{indent_str}ConcatSource::new(vec![")?;
    for child in self.children.iter() {
      writeln!(f, "{:indent$?},", child, indent = indent + 2)?;
    }
    write!(f, "{indent_str}]).boxed()")
  }
}

impl ConcatSource {
  /// Create a [ConcatSource] with [Source]s.
  pub fn new<S, T>(sources: S) -> Self
  where
    T: Source + 'static,
    S: IntoIterator<Item = T>,
  {
    let mut concat_soruce = ConcatSource::default();
    for source in sources {
      concat_soruce.add(source);
    }
    concat_soruce
  }

  fn children(&self) -> &Vec<BoxSource> {
    &self.children
  }

  /// Add a [Source] to concat.
  pub fn add<S: Source + 'static>(&mut self, source: S) {
    // First check if it's already a BoxSource containing a ConcatSource
    if let Some(box_source) = source.as_any().downcast_ref::<BoxSource>() {
      if let Some(concat_source) =
        box_source.as_ref().as_any().downcast_ref::<ConcatSource>()
      {
        // Extend with existing children (cheap clone due to Arc)
        self.children.extend(concat_source.children.iter().cloned());
        return;
      }
    }

    // Check if the source itself is a ConcatSource
    if let Some(concat_source) = source.as_any().downcast_ref::<ConcatSource>()
    {
      // Extend with existing children (cheap clone due to Arc)
      self.children.extend(concat_source.children.iter().cloned());
    } else {
      // Regular source - box it and add to children
      self.children.push(SourceExt::boxed(source));
    }
  }
}

impl Source for ConcatSource {
  fn source(&self) -> Cow<str> {
    let children = self.children();
    if children.len() == 1 {
      children[0].source()
    } else {
      let all = self.children().iter().map(|child| child.source()).collect();
      Cow::Owned(all)
    }
  }

  fn rope(&self) -> Rope<'_> {
    let children = self.children();
    if children.len() == 1 {
      children[0].rope()
    } else {
      let mut rope = Rope::new();
      for child in children {
        let child_rope = child.rope();
        rope.append(child_rope);
      }
      rope
    }
  }

  fn buffer(&self) -> Cow<[u8]> {
    let children = self.children();
    if children.len() == 1 {
      children[0].buffer()
    } else {
      let mut buffer = vec![];
      self.to_writer(&mut buffer).unwrap();
      Cow::Owned(buffer)
    }
  }

  fn size(&self) -> usize {
    self.children().iter().map(|child| child.size()).sum()
  }

  fn map(&self, options: &MapOptions) -> Option<SourceMap> {
    get_map(self, options)
  }

  fn to_writer(&self, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
    for child in self.children() {
      child.to_writer(writer)?;
    }
    Ok(())
  }
}

impl Hash for ConcatSource {
  fn hash<H: Hasher>(&self, state: &mut H) {
    "ConcatSource".hash(state);
    for child in self.children().iter() {
      child.hash(state);
    }
  }
}

impl PartialEq for ConcatSource {
  fn eq(&self, other: &Self) -> bool {
    self.children() == other.children()
  }
}
impl Eq for ConcatSource {}

impl StreamChunks for ConcatSource {
  fn stream_chunks<'a>(
    &'a self,
    options: &MapOptions,
    on_chunk: OnChunk<'_, 'a>,
    on_source: OnSource<'_, 'a>,
    on_name: OnName<'_, 'a>,
  ) -> crate::helpers::GeneratedInfo {
    if self.children().len() == 1 {
      return self.children[0]
        .stream_chunks(options, on_chunk, on_source, on_name);
    }
    let mut current_line_offset = 0;
    let mut current_column_offset = 0;
    let mut source_mapping: HashMap<Cow<str>, u32> = HashMap::default();
    let mut name_mapping: HashMap<Cow<str>, u32> = HashMap::default();
    let mut need_to_close_mapping = false;

    let source_index_mapping: RefCell<LinearMap<u32>> =
      RefCell::new(LinearMap::default());
    let name_index_mapping: RefCell<LinearMap<u32>> =
      RefCell::new(LinearMap::default());

    for item in self.children() {
      source_index_mapping.borrow_mut().clear();
      name_index_mapping.borrow_mut().clear();
      let mut last_mapping_line = 0;
      let GeneratedInfo {
        generated_line,
        generated_column,
      } = item.stream_chunks(
        options,
        &mut |chunk, mapping| {
          let line = mapping.generated_line + current_line_offset;
          let column = if mapping.generated_line == 1 {
            mapping.generated_column + current_column_offset
          } else {
            mapping.generated_column
          };
          if need_to_close_mapping {
            if mapping.generated_line != 1 || mapping.generated_column != 0 {
              on_chunk(
                None,
                Mapping {
                  generated_line: current_line_offset + 1,
                  generated_column: current_column_offset,
                  original: None,
                },
              );
            }
            need_to_close_mapping = false;
          }
          let result_source_index =
            mapping.original.as_ref().and_then(|original| {
              source_index_mapping
                .borrow()
                .get(&original.source_index)
                .copied()
            });
          let result_name_index = mapping
            .original
            .as_ref()
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
            if let (Some(result_source_index), Some(original)) =
              (result_source_index, &mapping.original)
            {
              on_chunk(
                None,
                Mapping {
                  generated_line: line,
                  generated_column: column,
                  original: Some(OriginalLocation {
                    source_index: result_source_index,
                    original_line: original.original_line,
                    original_column: original.original_column,
                    name_index: result_name_index,
                  }),
                },
              );
            }
          } else if let (Some(result_source_index), Some(original)) =
            (result_source_index, &mapping.original)
          {
            on_chunk(
              chunk,
              Mapping {
                generated_line: line,
                generated_column: column,
                original: Some(OriginalLocation {
                  source_index: result_source_index,
                  original_line: original.original_line,
                  original_column: original.original_column,
                  name_index: result_name_index,
                }),
              },
            );
          } else {
            on_chunk(
              chunk,
              Mapping {
                generated_line: line,
                generated_column: column,
                original: None,
              },
            );
          }
        },
        &mut |i, source, source_content| {
          let mut global_index = source_mapping.get(&source).copied();
          if global_index.is_none() {
            let len = source_mapping.len() as u32;
            source_mapping.insert(source.clone(), len);
            on_source(len, source, source_content);
            global_index = Some(len);
          }
          source_index_mapping
            .borrow_mut()
            .insert(i, global_index.unwrap());
        },
        &mut |i, name| {
          let mut global_index = name_mapping.get(&name).copied();
          if global_index.is_none() {
            let len = name_mapping.len() as u32;
            name_mapping.insert(name.clone(), len);
            on_name(len, name);
            global_index = Some(len);
          }
          name_index_mapping
            .borrow_mut()
            .insert(i, global_index.unwrap());
        },
      );
      if need_to_close_mapping && (generated_line != 1 || generated_column != 0)
      {
        on_chunk(
          None,
          Mapping {
            generated_line: current_line_offset + 1,
            generated_column: current_column_offset,
            original: None,
          },
        );
        need_to_close_mapping = false;
      }
      if generated_line > 1 {
        current_column_offset = generated_column;
      } else {
        current_column_offset += generated_column;
      }
      need_to_close_mapping = need_to_close_mapping
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
  use crate::{OriginalSource, RawBufferSource, RawSource, RawStringSource};

  use super::*;

  #[test]
  fn should_concat_two_sources() {
    let mut source = ConcatSource::new([
      RawSource::from("Hello World\n".to_string()).boxed(),
      OriginalSource::new(
        "console.log('test');\nconsole.log('test2');\n",
        "console.js",
      )
      .boxed(),
    ]);
    source.add(OriginalSource::new("Hello2\n", "hello.md"));

    let expected_source =
      "Hello World\nconsole.log('test');\nconsole.log('test2');\nHello2\n";
    assert_eq!(source.size(), 62);
    assert_eq!(source.source(), expected_source);
    assert_eq!(
      source.map(&MapOptions::new(false)).unwrap(),
      SourceMap::from_json(
        r#"{
          "version": 3,
          "mappings": ";AAAA;AACA;ACDA",
          "names": [],
          "sources": ["console.js", "hello.md"],
          "sourcesContent": [
            "console.log('test');\nconsole.log('test2');\n",
            "Hello2\n"
          ]
        }"#,
      )
      .unwrap()
    );
    assert_eq!(
      source.map(&MapOptions::default()).unwrap(),
      SourceMap::from_json(
        r#"{
          "version": 3,
          "mappings": ";AAAA;AACA;ACDA",
          "names": [],
          "sources": ["console.js", "hello.md"],
          "sourcesContent": [
            "console.log('test');\nconsole.log('test2');\n",
            "Hello2\n"
          ]
        }"#
      )
      .unwrap()
    );
  }

  #[test]
  fn should_concat_two_sources2() {
    let mut source = ConcatSource::new([
      RawStringSource::from("Hello World\n".to_string()).boxed(),
      OriginalSource::new(
        "console.log('test');\nconsole.log('test2');\n",
        "console.js",
      )
      .boxed(),
    ]);
    source.add(OriginalSource::new("Hello2\n", "hello.md"));

    let expected_source =
      "Hello World\nconsole.log('test');\nconsole.log('test2');\nHello2\n";
    assert_eq!(source.size(), 62);
    assert_eq!(source.source(), expected_source);
    assert_eq!(
      source.map(&MapOptions::new(false)).unwrap(),
      SourceMap::from_json(
        r#"{
          "version": 3,
          "mappings": ";AAAA;AACA;ACDA",
          "names": [],
          "sources": ["console.js", "hello.md"],
          "sourcesContent": [
            "console.log('test');\nconsole.log('test2');\n",
            "Hello2\n"
          ]
        }"#,
      )
      .unwrap()
    );
    assert_eq!(
      source.map(&MapOptions::default()).unwrap(),
      SourceMap::from_json(
        r#"{
          "version": 3,
          "mappings": ";AAAA;AACA;ACDA",
          "names": [],
          "sources": ["console.js", "hello.md"],
          "sourcesContent": [
            "console.log('test');\nconsole.log('test2');\n",
            "Hello2\n"
          ]
        }"#
      )
      .unwrap()
    );
  }

  #[test]
  fn should_concat_two_sources3() {
    let mut source = ConcatSource::new([
      RawBufferSource::from("Hello World\n".as_bytes()).boxed(),
      OriginalSource::new(
        "console.log('test');\nconsole.log('test2');\n",
        "console.js",
      )
      .boxed(),
    ]);
    source.add(OriginalSource::new("Hello2\n", "hello.md"));

    let expected_source =
      "Hello World\nconsole.log('test');\nconsole.log('test2');\nHello2\n";
    assert_eq!(source.size(), 62);
    assert_eq!(source.source(), expected_source);
    assert_eq!(
      source.map(&MapOptions::new(false)).unwrap(),
      SourceMap::from_json(
        r#"{
          "version": 3,
          "mappings": ";AAAA;AACA;ACDA",
          "names": [],
          "sources": ["console.js", "hello.md"],
          "sourcesContent": [
            "console.log('test');\nconsole.log('test2');\n",
            "Hello2\n"
          ]
        }"#,
      )
      .unwrap()
    );
    assert_eq!(
      source.map(&MapOptions::default()).unwrap(),
      SourceMap::from_json(
        r#"{
          "version": 3,
          "mappings": ";AAAA;AACA;ACDA",
          "names": [],
          "sources": ["console.js", "hello.md"],
          "sourcesContent": [
            "console.log('test');\nconsole.log('test2');\n",
            "Hello2\n"
          ]
        }"#
      )
      .unwrap()
    );
  }

  #[test]
  fn should_be_able_to_handle_strings_for_all_methods() {
    let mut source = ConcatSource::new([
      RawSource::from("Hello World\n".to_string()).boxed(),
      OriginalSource::new(
        "console.log('test');\nconsole.log('test2');\n",
        "console.js",
      )
      .boxed(),
    ]);
    let inner_source =
      ConcatSource::new([RawSource::from("("), "'string'".into(), ")".into()]);
    source.add(RawSource::from("console"));
    source.add(RawSource::from("."));
    source.add(RawSource::from("log"));
    source.add(inner_source);
    let expected_source =
      "Hello World\nconsole.log('test');\nconsole.log('test2');\nconsole.log('string')";
    let expected_map1 = SourceMap::from_json(
      r#"{
        "version": 3,
        "mappings": ";AAAA;AACA",
        "names": [],
        "sources": ["console.js"],
        "sourcesContent": ["console.log('test');\nconsole.log('test2');\n"]
      }"#,
    )
    .unwrap();
    assert_eq!(source.size(), 76);
    assert_eq!(source.source(), expected_source);
    assert_eq!(source.buffer(), expected_source.as_bytes());

    let map = source.map(&MapOptions::new(false)).unwrap();
    assert_eq!(map, expected_map1);

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
    let result_map = source.map(&MapOptions::default());
    let result_list_map = source.map(&MapOptions::new(false));

    assert_eq!(result_text, "Hello World\nHello World\n");
    assert!(result_map.is_none());
    assert!(result_list_map.is_none());
  }

  #[test]
  fn should_allow_to_concatenate_in_a_single_line() {
    let source = ConcatSource::new([
      OriginalSource::new("Hello", "hello.txt").boxed(),
      RawSource::from(" ").boxed(),
      OriginalSource::new("World ", "world.txt").boxed(),
      RawSource::from("is here\n").boxed(),
      OriginalSource::new("Hello\n", "hello.txt").boxed(),
      RawSource::from(" \n").boxed(),
      OriginalSource::new("World\n", "world.txt").boxed(),
      RawSource::from("is here").boxed(),
    ]);

    assert_eq!(
      source.map(&MapOptions::default()).unwrap(),
      SourceMap::from_json(
        r#"{
          "mappings": "AAAA,K,CCAA,M;ADAA;;ACAA",
          "names": [],
          "sources": ["hello.txt", "world.txt"],
          "sourcesContent": ["Hello", "World "],
          "version": 3
        }"#
      )
      .unwrap(),
    );
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
    assert!(source.map(&MapOptions::default()).is_none());
  }

  #[test]
  fn should_flatten_nested_concat_sources() {
    let inner_concat =
      ConcatSource::new([RawSource::from("Hello "), RawSource::from("World")]);

    let outer_concat = ConcatSource::new([
      inner_concat.boxed(),
      RawSource::from("!").boxed(),
      ConcatSource::new([RawSource::from(" How"), RawSource::from(" are")])
        .boxed(),
      RawSource::from(" you?").boxed(),
    ]);

    assert_eq!(outer_concat.source(), "Hello World! How are you?");
    // The key test: verify that nested ConcatSources are flattened
    // Should have 6 direct children instead of nested structure
    assert_eq!(outer_concat.children.len(), 6);
  }
}
