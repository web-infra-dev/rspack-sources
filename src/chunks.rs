use crate::{
  helpers::GeneratedInfo, rope::CharIndices, Mapping, OriginalLocation, Rope,
};

pub struct Chunks<'rope, 'text, Mappings> {
  source: &'rope Rope<'text>,
  char_indices: CharIndices<'rope, 'text>,

  pub current_generated_line: u32,
  pub current_generated_column: u32,

  tracking_generated_index: usize,
  tracking_generated_line: u32,
  tracking_generated_column: u32,
  tracking_mapping_original: Option<OriginalLocation>,

  mappings: Mappings,
  current_mapping: Option<Mapping>,

  generated_info: &'rope mut GeneratedInfo,
}

impl<'text, 'rope, Mappings> Chunks<'rope, 'text, Mappings>
where
  Mappings: Iterator<Item = Mapping> + 'rope,
{
  pub fn new(
    source: &'rope Rope<'text>,
    mut mappings: Mappings,
    generated_info: &'rope mut GeneratedInfo,
  ) -> Self {
    Chunks {
      current_generated_line: 1,
      current_generated_column: 0,
      current_mapping: mappings.next(),

      source,
      char_indices: source.char_indices(),
      mappings,

      tracking_generated_index: 0,
      tracking_generated_line: 1,
      tracking_generated_column: 0,
      tracking_mapping_original: None,

      generated_info,
    }
  }

  /// Gets the next valid mapping that is at or after the current position.
  ///
  /// This function skips mappings that are positioned before the current
  /// processing position to ensure chunks remain continuous and non-overlapping.
  ///
  /// This logic is consistent with webpack-sources to ensure continuous chunks.
  fn next_mapping(&mut self) -> Option<Mapping> {
    loop {
      match self.mappings.next() {
        Some(next_mapping) => {
          if next_mapping.generated_line > self.current_generated_line
            || (next_mapping.generated_line == self.current_generated_line
              && next_mapping.generated_column > self.current_generated_column)
          {
            break Some(next_mapping);
          }
        }
        None => break None,
      }
    }
  }
}

impl<'text, Mappings> Iterator for Chunks<'_, 'text, Mappings>
where
  Mappings: Iterator<Item = Mapping> + 'text,
{
  type Item = (Rope<'text>, Mapping);

  fn next(&mut self) -> Option<Self::Item> {
    loop {
      let Some((current_generated_index, char)) = self.char_indices.next()
      else {
        break;
      };

      // Check if current position matches a mapping
      if let Some(mapping) = self.current_mapping.take() {
        if mapping.generated_line == self.current_generated_line
          && mapping.generated_column == self.current_generated_column
        {
          let chunk = self
            .source
            .byte_slice(self.tracking_generated_index..current_generated_index);

          let chunk_mapping = Mapping {
            generated_line: self.tracking_generated_line,
            generated_column: self.tracking_generated_column,
            original: self.tracking_mapping_original.take(),
          };

          // Update tracking state to current mapping position
          self.tracking_generated_index = current_generated_index;
          self.tracking_generated_line = self.current_generated_line;
          self.tracking_generated_column = self.current_generated_column;
          self.tracking_mapping_original = mapping.original;

          self.current_mapping = self.next_mapping();

          if !chunk.is_empty() {
            self.current_generated_column += char.len_utf16() as u32;
            return Some((chunk, chunk_mapping));
          }
        } else {
          self.current_mapping = Some(mapping);
        }
      }

      if char == '\n' {
        let chunk = self.source.byte_slice(
          self.tracking_generated_index..current_generated_index + 1,
        );

        let chunk_mapping = Mapping {
          generated_line: self.tracking_generated_line,
          generated_column: self.tracking_generated_column,
          original: self.tracking_mapping_original.take(),
        };

        // Advance to next line
        self.tracking_generated_index = current_generated_index + 1;
        self.tracking_generated_line += 1;
        self.tracking_generated_column = 0;

        self.current_generated_line += 1;
        self.current_generated_column = 0;

        // Skip outdated mappings after line advance
        // This ensures mappings from previous lines don't interfere with current processing
        if let Some(mapping) = self.current_mapping.as_ref() {
          if mapping.generated_line < self.current_generated_line {
            self.current_mapping = self.next_mapping();
          }
        }

        return Some((chunk, chunk_mapping));
      } else {
        self.current_generated_column += char.len_utf16() as u32;
      }
    }

    let len = self.source.len();
    if self.tracking_generated_index < len {
      let chunk = self.source.byte_slice(self.tracking_generated_index..len);
      let chunk_mapping = Mapping {
        generated_line: self.tracking_generated_line,
        generated_column: self.tracking_generated_column,
        original: self.tracking_mapping_original.take(),
      };

      self.tracking_generated_index = len;

      return Some((chunk, chunk_mapping));
    }

    self.generated_info.generated_line = self.current_generated_line;
    self.generated_info.generated_column = self.current_generated_column;

    None
  }
}

#[cfg(test)]
#[rustfmt::skip::macros(assert_eq)]
mod tests {

  use crate::{decoder::MappingsDecoder, OriginalLocation};

  use super::*;

  #[test]
  fn test_babel_jsx_transformation() {
    let source = r#""use strict";

Object.defineProperty(exports, "__esModule", {
  value: true
});
exports.default = StaticPage;
function StaticPage(_ref) {
  let {
    data
  } = _ref;
  return /*#__PURE__*/React.createElement("div", null, data.foo);
}"#
      .into();
    let mut generated_info = GeneratedInfo {
      generated_line: 1,
      generated_column: 0,
    };
    let chunks =
      Chunks::new(&source, MappingsDecoder::new(";;;;;;AAAe,SAASA,UAAUA,CAAAC,IAAA,EAAW;EAAA,IAAV;IAAEC;EAAK,CAAC,GAAAD,IAAA;EACzC,oBAAOE,KAAA,CAAAC,aAAA,cAAMF,IAAI,CAACG,GAAS,CAAC;AAC9B"), &mut generated_info)
        .collect::<Vec<_>>();

    assert_eq!(
      chunks,
      [
        ("\"use strict\";\n".into(), Mapping { generated_line: 1, generated_column: 0, original: None }),
        ("\n".into(), Mapping { generated_line: 2, generated_column: 0, original: None }),
        ("Object.defineProperty(exports, \"__esModule\", {\n".into(), Mapping { generated_line: 3, generated_column: 0, original: None }),
        ("  value: true\n".into(), Mapping { generated_line: 4, generated_column: 0, original: None }),
        ("});\n".into(), Mapping { generated_line: 5, generated_column: 0, original: None }),
        ("exports.default = StaticPage;\n".into(), Mapping { generated_line: 6, generated_column: 0, original: None }),
        ("function ".into(), Mapping { generated_line: 7, generated_column: 0, original: Some(OriginalLocation { source_index: 0, original_line: 1, original_column: 15, name_index: None }) }),
        ("StaticPage".into(), Mapping { generated_line: 7, generated_column: 9, original: Some(OriginalLocation { source_index: 0, original_line: 1, original_column: 24, name_index: Some(0) }) }),
        ("(".into(), Mapping { generated_line: 7, generated_column: 19, original: Some(OriginalLocation { source_index: 0, original_line: 1, original_column: 34, name_index: Some(0) }) }),
        ("_ref".into(), Mapping { generated_line: 7, generated_column: 20, original: Some(OriginalLocation { source_index: 0, original_line: 1, original_column: 34, name_index: Some(1) }) }),
        (") ".into(), Mapping { generated_line: 7, generated_column: 24, original: Some(OriginalLocation { source_index: 0, original_line: 1, original_column: 34, name_index: None }) }),
        ("{\n".into(), Mapping { generated_line: 7, generated_column: 26, original: Some(OriginalLocation { source_index: 0, original_line: 1, original_column: 45, name_index: None }) }),
        ("  ".into(), Mapping { generated_line: 8, generated_column: 0, original: None }),
        ("let ".into(), Mapping { generated_line: 8, generated_column: 2, original: Some(OriginalLocation { source_index: 0, original_line: 1, original_column: 45, name_index: None }) }),
        ("{\n".into(), Mapping { generated_line: 8, generated_column: 6, original: Some(OriginalLocation { source_index: 0, original_line: 1, original_column: 35, name_index: None }) }),
        ("    ".into(), Mapping { generated_line: 9, generated_column: 0, original: None }),
        ("data\n".into(), Mapping { generated_line: 9, generated_column: 4, original: Some(OriginalLocation { source_index: 0, original_line: 1, original_column: 37, name_index: Some(2) }) }),
        ("  ".into(), Mapping { generated_line: 10, generated_column: 0, original: None }),
        ("}".into(), Mapping { generated_line: 10, generated_column: 2, original: Some(OriginalLocation { source_index: 0, original_line: 1, original_column: 42, name_index: None }) }),
        (" = ".into(), Mapping { generated_line: 10, generated_column: 3, original: Some(OriginalLocation { source_index: 0, original_line: 1, original_column: 43, name_index: None }) }),
        ("_ref".into(), Mapping { generated_line: 10, generated_column: 6, original: Some(OriginalLocation { source_index: 0, original_line: 1, original_column: 43, name_index: Some(1) }) }),
        (";\n".into(), Mapping { generated_line: 10, generated_column: 10, original: Some(OriginalLocation { source_index: 0, original_line: 1, original_column: 43, name_index: None }) }),
        ("  ".into(), Mapping { generated_line: 11, generated_column: 0, original: None }),
        ("return /*#__PURE__*/".into(), Mapping { generated_line: 11, generated_column: 2, original: Some(OriginalLocation { source_index: 0, original_line: 2, original_column: 2, name_index: None }) }),
        ("React".into(), Mapping { generated_line: 11, generated_column: 22, original: Some(OriginalLocation { source_index: 0, original_line: 2, original_column: 9, name_index: Some(3) }) }),
        (".".into(), Mapping { generated_line: 11, generated_column: 27, original: Some(OriginalLocation { source_index: 0, original_line: 2, original_column: 9, name_index: None }) }),
        ("createElement".into(), Mapping { generated_line: 11, generated_column: 28, original: Some(OriginalLocation { source_index: 0, original_line: 2, original_column: 9, name_index: Some(4) }) }),
        ("(\"div\", null, ".into(), Mapping { generated_line: 11, generated_column: 41, original: Some(OriginalLocation { source_index: 0, original_line: 2, original_column: 9, name_index: None }) }),
        ("data".into(), Mapping { generated_line: 11, generated_column: 55, original: Some(OriginalLocation { source_index: 0, original_line: 2, original_column: 15, name_index: Some(2) }) }),
        (".".into(), Mapping { generated_line: 11, generated_column: 59, original: Some(OriginalLocation { source_index: 0, original_line: 2, original_column: 19, name_index: None }) }),
        ("foo".into(), Mapping { generated_line: 11, generated_column: 60, original: Some(OriginalLocation { source_index: 0, original_line: 2, original_column: 20, name_index: Some(5) }) }),
        (")".into(), Mapping { generated_line: 11, generated_column: 63, original: Some(OriginalLocation { source_index: 0, original_line: 2, original_column: 29, name_index: None }) }),
        (";\n".into(), Mapping { generated_line: 11, generated_column: 64, original: Some(OriginalLocation { source_index: 0, original_line: 2, original_column: 30, name_index: None }) }),
        ("}".into(), Mapping { generated_line: 12, generated_column: 0, original: Some(OriginalLocation { source_index: 0, original_line: 3, original_column: 0, name_index: None }) })
      ]
    );
  }
}
