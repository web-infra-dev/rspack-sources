use std::{
  borrow::Cow,
  hash::{Hash, Hasher},
};

use crate::{
  helpers::{
    get_generated_source_info, get_map, split_into_lines,
    split_into_potential_tokens, GeneratedInfo, OnChunk, OnName, OnSource,
    StreamChunks,
  },
  source::{Mapping, OriginalLocation},
  MapOptions, Source, SourceMap,
};

/// Represents source code, it will create source map for the source code,
/// but the source map is created by splitting the source code at typical
/// statement borders (`;`, `{`, `}`).
///
/// - [webpack-sources docs](https://github.com/webpack/webpack-sources/#originalsource).
///
/// ```
/// use rspack_sources::{OriginalSource, MapOptions, Source};
///
/// let input = "if (hello()) { world(); hi(); there(); } done();\nif (hello()) { world(); hi(); there(); } done();";
/// let source = OriginalSource::new(input, "file.js");
/// assert_eq!(source.source(), input);
/// assert_eq!(
///   source.map(&MapOptions::default()).unwrap().mappings(),
///   "AAAA,eAAe,SAAS,MAAM,WAAW;AACzC,eAAe,SAAS,MAAM,WAAW",
/// );
/// assert_eq!(
///   source.map(&MapOptions::new(false)).unwrap().mappings(),
///   "AAAA;AACA",
/// );
/// ```
#[derive(Clone, Eq)]
pub struct OriginalSource {
  value: String,
  name: String,
}

impl OriginalSource {
  /// Create a [OriginalSource].
  pub fn new(value: impl Into<String>, name: impl Into<String>) -> Self {
    Self {
      value: value.into(),
      name: name.into(),
    }
  }
}

impl Source for OriginalSource {
  fn source(&self) -> Cow<str> {
    Cow::Borrowed(&self.value)
  }

  fn buffer(&self) -> Cow<[u8]> {
    Cow::Borrowed(self.value.as_bytes())
  }

  fn size(&self) -> usize {
    self.value.len()
  }

  fn map(&self, options: &MapOptions) -> Option<SourceMap> {
    get_map(self, options)
  }

  fn to_writer(&self, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
    writer.write_all(self.value.as_bytes())
  }
}

impl Hash for OriginalSource {
  fn hash<H: Hasher>(&self, state: &mut H) {
    "OriginalSource".hash(state);
    self.buffer().hash(state);
    self.name.hash(state);
  }
}

impl PartialEq for OriginalSource {
  fn eq(&self, other: &Self) -> bool {
    if std::ptr::eq(self, other) {
      return true;
    }
    self.value == other.value && self.name == other.name
  }
}

impl std::fmt::Debug for OriginalSource {
  fn fmt(
    &self,
    f: &mut std::fmt::Formatter<'_>,
  ) -> Result<(), std::fmt::Error> {
    f.debug_struct("OriginalSource")
      .field("name", &self.name)
      .field("value", &self.value.chars().take(50).collect::<String>())
      .finish()
  }
}

impl<'a> StreamChunks<'a> for OriginalSource {
  fn stream_chunks(
    &'a self,
    options: &MapOptions,
    on_chunk: OnChunk<'_, 'a>,
    on_source: OnSource<'_, 'a>,
    _on_name: OnName,
  ) -> crate::helpers::GeneratedInfo {
    on_source(0, Cow::Borrowed(&self.name), Some(&self.value));
    if options.columns {
      // With column info we need to read all lines and split them
      let mut line = 1;
      let mut column = 0;
      for token in split_into_potential_tokens(&self.value) {
        let is_end_of_line = token.ends_with('\n');
        if is_end_of_line && token.len() == 1 {
          if !options.final_source {
            on_chunk(
              Some(Cow::Borrowed(token)),
              Mapping {
                generated_line: line,
                generated_column: column,
                original: None,
              },
            );
          }
        } else {
          on_chunk(
            (!options.final_source).then_some(Cow::Borrowed(token)),
            Mapping {
              generated_line: line,
              generated_column: column,
              original: Some(OriginalLocation {
                source_index: 0,
                original_line: line,
                original_column: column,
                name_index: None,
              }),
            },
          );
        }
        if is_end_of_line {
          line += 1;
          column = 0;
        } else {
          column += token.len() as u32;
        }
      }
      GeneratedInfo {
        generated_line: line,
        generated_column: column,
      }
    } else if options.final_source {
      // Without column info and with final source we only
      // need meta info to generate mapping
      let result = get_generated_source_info(&self.value);
      if result.generated_column == 0 {
        for line in 1..result.generated_line {
          on_chunk(
            None,
            Mapping {
              generated_line: line,
              generated_column: 0,
              original: Some(OriginalLocation {
                source_index: 0,
                original_line: line,
                original_column: 0,
                name_index: None,
              }),
            },
          );
        }
      } else {
        for line in 1..=result.generated_line {
          on_chunk(
            None,
            Mapping {
              generated_line: line,
              generated_column: 0,
              original: Some(OriginalLocation {
                source_index: 0,
                original_line: line,
                original_column: 0,
                name_index: None,
              }),
            },
          );
        }
      }
      result
    } else {
      // Without column info, but also without final source
      // we need to split source by lines
      let mut line = 1;
      let mut last_line = None;
      for l in split_into_lines(&self.value) {
        on_chunk(
          (!options.final_source).then_some(Cow::Borrowed(l)),
          Mapping {
            generated_line: line,
            generated_column: 0,
            original: Some(OriginalLocation {
              source_index: 0,
              original_line: line,
              original_column: 0,
              name_index: None,
            }),
          },
        );
        line += 1;
        last_line = Some(l);
      }
      if let Some(last_line) =
        last_line.filter(|last_line| !last_line.ends_with('\n'))
      {
        GeneratedInfo {
          generated_line: line - 1,
          generated_column: last_line.len() as u32,
        }
      } else {
        GeneratedInfo {
          generated_line: line,
          generated_column: 0,
        }
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use crate::{ConcatSource, ReplaceSource, SourceExt};

  use super::*;

  #[test]
  fn should_handle_multiline_string() {
    let source = OriginalSource::new("Line1\n\nLine3\n", "file.js");
    let result_text = source.source();
    let result_map = source.map(&MapOptions::default()).unwrap();
    let result_list_map = source.map(&MapOptions::new(false)).unwrap();

    assert_eq!(result_text, "Line1\n\nLine3\n");
    assert_eq!(
      result_map
        .sources()
        .into_iter()
        .map(|s| s.as_ref())
        .collect::<Vec<_>>(),
      &["file.js"]
    );
    assert_eq!(
      result_list_map
        .sources()
        .into_iter()
        .map(|s| s.as_ref())
        .collect::<Vec<_>>(),
      ["file.js"]
    );
    assert_eq!(
      result_map
        .sources_content()
        .into_iter()
        .map(|s| s.as_ref())
        .collect::<Vec<_>>(),
      ["Line1\n\nLine3\n"],
    );
    assert_eq!(
      result_list_map
        .sources_content()
        .into_iter()
        .map(|s| s.as_ref())
        .collect::<Vec<_>>(),
      ["Line1\n\nLine3\n"],
    );
    assert_eq!(result_map.mappings(), "AAAA;;AAEA");
    assert_eq!(result_list_map.mappings(), "AAAA;AACA;AACA");
  }

  #[test]
  fn should_handle_empty_string() {
    let source = OriginalSource::new("", "file.js");
    let result_text = source.source();
    let result_map = source.map(&MapOptions::default());
    let result_list_map = source.map(&MapOptions::new(false));

    assert_eq!(result_text, "");
    assert!(result_map.is_none());
    assert!(result_list_map.is_none());
  }

  #[test]
  fn should_omit_mappings_for_columns_with_node() {
    let source = OriginalSource::new("Line1\n\nLine3\n", "file.js");
    let result_map = source.map(&MapOptions::new(false)).unwrap();
    assert_eq!(result_map.mappings(), "AAAA;AACA;AACA");
  }

  #[test]
  fn should_return_the_correct_size_for_binary_files() {
    let source = OriginalSource::new(
      String::from_utf8(vec![0; 256]).unwrap(),
      "file.wasm",
    );
    assert_eq!(source.size(), 256);
  }

  #[test]
  fn should_return_the_correct_size_for_unicode_files() {
    let source = OriginalSource::new("😋", "file.js");
    assert_eq!(source.size(), 4);
  }

  #[test]
  fn should_split_code_into_statements() {
    let input = "if (hello()) { world(); hi(); there(); } done();\nif (hello()) { world(); hi(); there(); } done();";
    let source = OriginalSource::new(input, "file.js");
    assert_eq!(source.source(), input);
    assert_eq!(
      source.map(&MapOptions::default()).unwrap().mappings(),
      "AAAA,eAAe,SAAS,MAAM,WAAW;AACzC,eAAe,SAAS,MAAM,WAAW",
    );
    assert_eq!(
      source.map(&MapOptions::new(false)).unwrap().mappings(),
      "AAAA;AACA",
    );
  }

  // Fix https://github.com/web-infra-dev/rspack/issues/6793
  #[test]
  fn fix_rspack_issue_6793() {
    let code1 = "hello\n\n";
    let source1 = OriginalSource::new(code1, "hello.txt").boxed();
    let source1 = ReplaceSource::new(source1);

    let code2 = "world";
    let source2 = OriginalSource::new(code2, "world.txt");

    let concat = ConcatSource::new([source1.boxed(), source2.boxed()]);
    let map = concat.map(&MapOptions::new(false)).unwrap();
    assert_eq!(map.mappings(), "AAAA;AACA;ACDA",);
  }
}
