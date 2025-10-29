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
  fn advance_to_next_mapping(&mut self) {
    #[allow(clippy::while_let_on_iterator)]
    while let Some(mapping) = self.mappings.next() {
      if mapping.generated_line > self.current_generated_line
        || (mapping.generated_line == self.current_generated_line
          && mapping.generated_column > self.current_generated_column)
      {
        self.current_mapping = Some(mapping);
        return;
      }
      // Skip outdated mapping - continue loop
    }
    self.current_mapping = None;
  }

  /// Skip mappings that have "backtracked" (are located before the current
  /// processing position). This avoids emitting overlapping/retrograde mappings.
  fn skip_backtracked_mappings(&mut self) {
    if let Some(mapping) = self.current_mapping.as_ref() {
      if mapping.generated_line < self.current_generated_line {
        self.advance_to_next_mapping();
      }
    }
  }
}

impl<'text, Mappings> Iterator for Chunks<'_, 'text, Mappings>
where
  Mappings: Iterator<Item = Mapping> + 'text,
{
  type Item = (Rope<'text>, Mapping);

  #[inline(always)]
  fn next(&mut self) -> Option<Self::Item> {
    while let Some((current_generated_index, char)) = self.char_indices.next() {
      // Check if current position matches a mapping
      if let Some(mapping) = self.current_mapping.as_ref() {
        if mapping.generated_line == self.current_generated_line
          && mapping.generated_column == self.current_generated_column
        {
          #[allow(unsafe_code)]
          let chunk = unsafe {
            self.source.byte_slice_unchecked(
              self.tracking_generated_index..current_generated_index,
            )
          };

          let chunk_mapping = Mapping {
            generated_line: self.tracking_generated_line,
            generated_column: self.tracking_generated_column,
            original: self.tracking_mapping_original.take(),
          };

          // Update tracking state to current mapping position
          self.tracking_generated_index = current_generated_index;
          self.tracking_generated_line = self.current_generated_line;
          self.tracking_generated_column = self.current_generated_column;
          self.tracking_mapping_original = mapping.original.clone();

          self.advance_to_next_mapping();

          if !chunk.is_empty() {
            if char == '\n' {
              // Advance to next line
              self.tracking_generated_index = current_generated_index + 1;
              self.tracking_generated_line += 1;
              self.tracking_generated_column = 0;

              self.current_generated_line += 1;
              self.current_generated_column = 0;

              self.skip_backtracked_mappings();
            } else {
              self.current_generated_column += char.len_utf16() as u32;
            }
            return Some((chunk, chunk_mapping));
          }
        }
      }

      if char == '\n' {
        #[allow(unsafe_code)]
        let chunk = unsafe {
          self.source.byte_slice_unchecked(
            self.tracking_generated_index..current_generated_index + 1,
          )
        };

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

        self.skip_backtracked_mappings();

        return Some((chunk, chunk_mapping));
      } else {
        self.current_generated_column += char.len_utf16() as u32;
      }
    }

    // Emit final chunk if any content remains
    let len = self.source.len();
    if self.tracking_generated_index < len {
      #[allow(unsafe_code)]
      let chunk = unsafe {
        self
          .source
          .byte_slice_unchecked(self.tracking_generated_index..len)
      };
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
    assert_eq!(generated_info, GeneratedInfo { generated_line: 12, generated_column: 1 });
  }

  #[test]
  fn test_swc_jsx_transformation() {
    let source = r#"export default function StaticPage(param) {
    var data = param.data;
    return /*#__PURE__*/ React.createElement("div", null, data.foo);
}

"#
    .into();
    let mut generated_info = GeneratedInfo {
      generated_line: 1,
      generated_column: 0,
    };
    let chunks =
      Chunks::new(&source, MappingsDecoder::new("AAAA,eAAe,SAASA,WAAW,KAAQ;QAAR,AAAEC,OAAF,MAAEA;IACnC,qBAAO,oBAACC,aAAKD,KAAKE,GAAG;AACvB"), &mut generated_info)
        .collect::<Vec<_>>();

    assert_eq!(
      chunks,
      [
        ("export default ".into(), Mapping { generated_line: 1, generated_column: 0, original: Some(OriginalLocation { source_index: 0, original_line: 1, original_column: 0, name_index: None }) }),
        ("function ".into(), Mapping { generated_line: 1, generated_column: 15, original: Some(OriginalLocation { source_index: 0, original_line: 1, original_column: 15, name_index: None }) }),
        ("StaticPage(".into(), Mapping { generated_line: 1, generated_column: 24, original: Some(OriginalLocation { source_index: 0, original_line: 1, original_column: 24, name_index: Some(0) }) }),
        ("param".into(), Mapping { generated_line: 1, generated_column: 35, original: Some(OriginalLocation { source_index: 0, original_line: 1, original_column: 35, name_index: None }) }),
        (") {\n".into(), Mapping { generated_line: 1, generated_column: 40, original: Some(OriginalLocation { source_index: 0, original_line: 1, original_column: 43, name_index: None }) }),
        ("    var ".into(), Mapping { generated_line: 2, generated_column: 0, original: None }),
        ("data = ".into(), Mapping { generated_line: 2, generated_column: 8, original: Some(OriginalLocation { source_index: 0, original_line: 1, original_column: 35, name_index: None }) }),
        ("param.".into(), Mapping { generated_line: 2, generated_column: 15, original: Some(OriginalLocation { source_index: 0, original_line: 1, original_column: 35, name_index: None }) }),
        ("data;\n".into(), Mapping { generated_line: 2, generated_column: 21, original: Some(OriginalLocation { source_index: 0, original_line: 1, original_column: 37, name_index: Some(1) }) }),
        ("    ".into(), Mapping { generated_line: 3, generated_column: 0, original: None }),
        ("return /*#__PURE__*/ ".into(), Mapping { generated_line: 3, generated_column: 4, original: Some(OriginalLocation { source_index: 0, original_line: 2, original_column: 2, name_index: None }) }),
        ("React.createElement(".into(), Mapping { generated_line: 3, generated_column: 25, original: Some(OriginalLocation { source_index: 0, original_line: 2, original_column: 9, name_index: None }) }),
        ("\"div\", null, ".into(), Mapping { generated_line: 3, generated_column: 45, original: Some(OriginalLocation { source_index: 0, original_line: 2, original_column: 10, name_index: Some(2) }) }),
        ("data.".into(), Mapping { generated_line: 3, generated_column: 58, original: Some(OriginalLocation { source_index: 0, original_line: 2, original_column: 15, name_index: Some(1) }) }),
        ("foo".into(), Mapping { generated_line: 3, generated_column: 63, original: Some(OriginalLocation { source_index: 0, original_line: 2, original_column: 20, name_index: Some(3) }) }),
        (");\n".into(), Mapping { generated_line: 3, generated_column: 66, original: Some(OriginalLocation { source_index: 0, original_line: 2, original_column: 23, name_index: None }) }),
        ("}\n".into(), Mapping { generated_line: 4, generated_column: 0, original: Some(OriginalLocation { source_index: 0, original_line: 3, original_column: 0, name_index: None }) }),
        ("\n".into(), Mapping { generated_line: 5, generated_column: 0, original: None })
      ]
    );
    assert_eq!(generated_info, GeneratedInfo { generated_line: 6, generated_column: 0 });
  }

  #[test]
  fn test_swc_jsx_minification() {
    // export default function StaticPage({ data }) {
    //   return <div>{data.foo}</div>
    // }
    let source = r#"export default function e(e){var t=e.data;return React.createElement("div",null,t.foo)}
"#
      .into();
    let mut generated_info = GeneratedInfo {
      generated_line: 1,
      generated_column: 0,
    };
    let chunks =
      Chunks::new(&source, MappingsDecoder::new("AAAA,eAAe,SAASA,EAAW,CAAQ,MAAR,AAAEC,EAAF,EAAEA,KACnC,OAAO,oBAACC,WAAKD,EAAKE,GAAG,CACvB"), &mut generated_info)
        .collect::<Vec<_>>();

    assert_eq!(
      chunks,
      [
        ("export default ".into(), Mapping { generated_line: 1, generated_column: 0, original: Some(OriginalLocation { source_index: 0, original_line: 1, original_column: 0, name_index: None }) }),
        ("function ".into(), Mapping { generated_line: 1, generated_column: 15, original: Some(OriginalLocation { source_index: 0, original_line: 1, original_column: 15, name_index: None }) }),
        ("e(".into(), Mapping { generated_line: 1, generated_column: 24, original: Some(OriginalLocation { source_index: 0, original_line: 1, original_column: 24, name_index: Some(0) }) }),
        ("e".into(), Mapping { generated_line: 1, generated_column: 26, original: Some(OriginalLocation { source_index: 0, original_line: 1, original_column: 35, name_index: None }) }),
        ("){var ".into(), Mapping { generated_line: 1, generated_column: 27, original: Some(OriginalLocation { source_index: 0, original_line: 1, original_column: 43, name_index: None }) }),
        ("t=".into(), Mapping { generated_line: 1, generated_column: 33, original: Some(OriginalLocation { source_index: 0, original_line: 1, original_column: 35, name_index: None }) }),
        ("e.".into(), Mapping { generated_line: 1, generated_column: 35, original: Some(OriginalLocation { source_index: 0, original_line: 1, original_column: 35, name_index: None }) }),
        ("data;".into(), Mapping { generated_line: 1, generated_column: 37, original: Some(OriginalLocation { source_index: 0, original_line: 1, original_column: 37, name_index: Some(1) }) }),
        ("return ".into(), Mapping { generated_line: 1, generated_column: 42, original: Some(OriginalLocation { source_index: 0, original_line: 2, original_column: 2, name_index: None }) }),
        ("React.createElement(".into(), Mapping { generated_line: 1, generated_column: 49, original: Some(OriginalLocation { source_index: 0, original_line: 2, original_column: 9, name_index: None }) }),
        ("\"div\",null,".into(), Mapping { generated_line: 1, generated_column: 69, original: Some(OriginalLocation { source_index: 0, original_line: 2, original_column: 10, name_index: Some(2) }) }),
        ("t.".into(), Mapping { generated_line: 1, generated_column: 80, original: Some(OriginalLocation { source_index: 0, original_line: 2, original_column: 15, name_index: Some(1) }) }),
        ("foo".into(), Mapping { generated_line: 1, generated_column: 82, original: Some(OriginalLocation { source_index: 0, original_line: 2, original_column: 20, name_index: Some(3) }) }),
        (")".into(), Mapping { generated_line: 1, generated_column: 85, original: Some(OriginalLocation { source_index: 0, original_line: 2, original_column: 23, name_index: None }) }),
        ("}\n".into(), Mapping { generated_line: 1, generated_column: 86, original: Some(OriginalLocation { source_index: 0, original_line: 3, original_column: 0, name_index: None }) })
      ]
    );
    assert_eq!(generated_info, GeneratedInfo { generated_line: 2, generated_column: 0 });
  }
}
