use std::{
  any::Any,
  borrow::Cow,
  cell::RefCell,
  hash::{Hash, Hasher},
  mem,
  sync::{
    atomic::{AtomicBool, Ordering},
    Mutex, MutexGuard,
  },
};

use rustc_hash::FxHashMap as HashMap;
use rustc_hash::FxHashSet as HashSet;

use crate::{
  helpers::{get_map, GeneratedInfo, OnChunk, OnName, OnSource, StreamChunks},
  source::{Mapping, OriginalLocation},
  BoxSource, MapOptions, RawSource, Source, SourceExt, SourceMap,
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
#[derive(Debug, Default)]
pub struct ConcatSource {
  children: Mutex<Vec<BoxSource>>,
  is_optimized: AtomicBool,
}

impl Clone for ConcatSource {
  fn clone(&self) -> Self {
    Self {
      children: Mutex::new(self.children.lock().unwrap().clone()),
      is_optimized: AtomicBool::new(self.is_optimized()),
    }
  }
}

fn downcast_ref<R: 'static>(source: &dyn Any) -> Option<&R> {
  if let Some(source) = source.downcast_ref::<BoxSource>() {
    source.as_any().downcast_ref::<R>()
  } else {
    source.downcast_ref::<R>()
  }
}

impl ConcatSource {
  /// Create a [ConcatSource] with [Source]s.
  pub fn new<S, T>(sources: S) -> Self
  where
    T: Source + 'static,
    S: IntoIterator<Item = T>,
  {
    // Flatten the children
    let children = sources
      .into_iter()
      .flat_map(|source| {
        if let Some(concat_source) =
          downcast_ref::<ConcatSource>(source.as_any())
        {
          concat_source.children().clone()
        } else {
          vec![source.boxed()]
        }
      })
      .collect::<Vec<_>>();
    let is_optimized = AtomicBool::new(children.is_empty());
    Self {
      children: Mutex::new(children),
      is_optimized,
    }
  }

  /// Add a [Source] to concat.
  pub fn add<S: Source + 'static>(&mut self, source: S) {
    if let Some(concat_source) = source.as_any().downcast_ref::<ConcatSource>()
    {
      self
        .children
        .lock()
        .unwrap()
        .extend(concat_source.children().clone());
    } else {
      self.children.lock().unwrap().push(source.boxed());
    }
    self.optimize_off();
  }

  fn children(&self) -> MutexGuard<Vec<BoxSource>> {
    self.optimize();
    self.children.lock().unwrap()
  }

  fn is_optimized(&self) -> bool {
    self.is_optimized.load(Ordering::SeqCst)
  }

  fn optimize_off(&self) {
    self.is_optimized.store(false, Ordering::SeqCst);
  }

  fn optimize(&self) {
    if self.is_optimized() {
      return;
    }
    let mut children = self.children.lock().unwrap();
    let new_children = ConcatSourceOptimizer::default().optimize(&children);
    *children = new_children;
    self.is_optimized.store(true, Ordering::SeqCst);
  }
}

impl Source for ConcatSource {
  fn source(&self) -> Cow<str> {
    let all = self.children().iter().map(|child| child.source()).collect();
    Cow::Owned(all)
  }

  fn buffer(&self) -> Cow<[u8]> {
    let all = self
      .children()
      .iter()
      .map(|child| child.buffer())
      .collect::<Vec<_>>()
      .concat();
    Cow::Owned(all)
  }

  fn size(&self) -> usize {
    self.children().iter().map(|child| child.size()).sum()
  }

  fn map(&self, options: &MapOptions) -> Option<SourceMap> {
    get_map(self, options)
  }

  fn to_writer(&self, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
    for child in self.children().iter() {
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
    *self.children() == *other.children()
  }
}
impl Eq for ConcatSource {}

impl StreamChunks for ConcatSource {
  fn stream_chunks(
    &self,
    options: &MapOptions,
    on_chunk: OnChunk,
    on_source: OnSource,
    on_name: OnName,
  ) -> crate::helpers::GeneratedInfo {
    let children = self.children();
    if children.len() == 1 {
      return children[0].stream_chunks(options, on_chunk, on_source, on_name);
    }
    let mut current_line_offset = 0;
    let mut current_column_offset = 0;
    let mut source_mapping: HashMap<String, u32> = HashMap::default();
    let mut name_mapping: HashMap<String, u32> = HashMap::default();
    let mut need_to_cloas_mapping = false;
    for item in children.iter() {
      let source_index_mapping: RefCell<HashMap<u32, u32>> =
        RefCell::new(HashMap::default());
      let name_index_mapping: RefCell<HashMap<u32, u32>> =
        RefCell::new(HashMap::default());
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
          if need_to_cloas_mapping {
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
            need_to_cloas_mapping = false;
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
          let mut global_index = source_mapping.get(source).copied();
          if global_index.is_none() {
            let len = source_mapping.len() as u32;
            source_mapping.insert(source.to_owned(), len);
            on_source(len, source, source_content);
            global_index = Some(len);
          }
          source_index_mapping
            .borrow_mut()
            .insert(i, global_index.unwrap());
        },
        &mut |i, name| {
          let mut global_index = name_mapping.get(name).copied();
          if global_index.is_none() {
            let len = name_mapping.len() as u32;
            name_mapping.insert(name.to_owned(), len);
            on_name(len, name);
            global_index = Some(len);
          }
          name_index_mapping
            .borrow_mut()
            .insert(i, global_index.unwrap());
        },
      );
      if need_to_cloas_mapping && (generated_line != 1 || generated_column != 0)
      {
        on_chunk(
          None,
          Mapping {
            generated_line: current_line_offset + 1,
            generated_column: current_column_offset,
            original: None,
          },
        );
        need_to_cloas_mapping = false;
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

#[derive(Debug, Default)]
struct ConcatSourceOptimizer {
  new_children: Vec<BoxSource>,
  current_string: Option<String>,
  current_raw_sources: CurrentRawSources,
  strings_as_raw_sources: HashSet<BoxSource>,
}

#[derive(Debug, Default, PartialEq)]
enum CurrentRawSources {
  #[default]
  None,
  String(String),
  Array(Vec<String>),
}

impl ConcatSourceOptimizer {
  #[must_use]
  fn optimize(mut self, children: &[BoxSource]) -> Vec<BoxSource> {
    for child in children.iter() {
      if let Some(raw_source) =
        downcast_ref::<RawSource>(child.as_any()).filter(|s| s.is_string())
      {
        if let Some(current_string) = &mut self.current_string {
          current_string.push_str(raw_source.source().as_ref());
        } else {
          self.current_string.replace(raw_source.source().to_string());
        }
      } else {
        if let Some(current_string) = self.current_string.take() {
          self.add_string_to_raw_sources(current_string);
        }
        if self.strings_as_raw_sources.contains(child) {
          // self.add_source_to_raw_sources(child);
        } else {
          if self.current_raw_sources != CurrentRawSources::None {
            self.merge_raw_sources();
            self.current_raw_sources = CurrentRawSources::None;
          }
          self.new_children.push(child.clone());
        }
      }
    }
    if let Some(current_string) = self.current_string.take() {
      self.add_string_to_raw_sources(current_string);
    }
    if self.current_raw_sources != CurrentRawSources::None {
      self.merge_raw_sources();
    }
    self.new_children
  }

  fn add_string_to_raw_sources(&mut self, string: String) {
    self.current_raw_sources = match mem::take(&mut self.current_raw_sources) {
      CurrentRawSources::None => CurrentRawSources::String(string),
      CurrentRawSources::Array(array) => {
        let mut array = array;
        array.push(string);
        CurrentRawSources::Array(array)
      }
      CurrentRawSources::String(existing_string) => {
        CurrentRawSources::Array(vec![existing_string, string])
      }
    };
  }

  // fn add_source_to_raw_sources(&mut self, source: &BoxSource) {
  // self.current_raw_sources = match mem::take(&mut self.current_raw_sources) {
  // CurrentRawSources::None => {
  // CurrentRawSources::String(source)

  // }
  // CurrentRawSources::Array(array) => {
  // let raw_source = RawSource::from(array.into_iter().collect::<String>());
  // // stringsAsRawSources.add(rawSource);
  // self.new_children.push(raw_source.boxed());
  // }
  // CurrentRawSources::String(existing_string) => {
  // let raw_source = RawSource::from(existing_string);
  // // stringsAsRawSources.add(rawSource);
  // self.new_children.push(raw_source.boxed());
  // }
  // };
  // }

  fn merge_raw_sources(&mut self) {
    match mem::take(&mut self.current_raw_sources) {
      CurrentRawSources::Array(array) => {
        let raw_source = RawSource::from(array.into_iter().collect::<String>());
        // stringsAsRawSources.add(rawSource);
        self.new_children.push(raw_source.boxed());
      }
      CurrentRawSources::String(existing_string) => {
        let raw_source = RawSource::from(existing_string);
        // stringsAsRawSources.add(rawSource);
        self.new_children.push(raw_source.boxed());
      }
      CurrentRawSources::None => {}
    };
  }
}

#[cfg(test)]
mod tests {
  use crate::{OriginalSource, RawSource};

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
  fn optimize() {
    let source = ConcatSource::new([
      RawSource::from("a").boxed(),
      ConcatSource::new([
        RawSource::from("b"),
        RawSource::from("c"),
        RawSource::from("d"),
      ])
      .boxed(),
      RawSource::from("e").boxed(),
    ]);
    assert_eq!(source.source(), "abcde");
    assert_eq!(source.children.lock().unwrap().len(), 1);
    assert!(source.is_optimized());
  }
}
