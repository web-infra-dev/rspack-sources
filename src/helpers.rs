use crate::{
  source::{Mapping, Mappings},
  vlq::encode,
  MapOptions, SourceMap,
};

pub fn get_map<S: StreamChunks>(
  stream: &S,
  options: MapOptions,
) -> Option<SourceMap> {
  let mut mappings = Vec::new();
  let mut sources = Vec::new();
  let mut sources_content = Vec::new();
  let mut names = Vec::new();
  stream.stream_chunks(
    &MapOptions {
      columns: options.columns,
      final_source: true,
    },
    &mut |mapping| {
      mappings.push(mapping);
    },
    &mut |source_index, source: &str, source_content: Option<&str>| {
      let source_index = source_index as usize;
      while sources.len() <= source_index {
        sources.push(None);
      }
      sources[source_index] = Some(source.to_string());
      if let Some(source_content) = source_content {
        while sources_content.len() <= source_index {
          sources_content.push(None);
        }
        sources_content[source_index] = Some(source_content.to_string());
      }
    },
    &mut |name_index, name: &str| {
      let name_index = name_index as usize;
      while names.len() <= name_index {
        names.push(None);
      }
      names[name_index] = Some(name.to_string());
    },
  );
  (!mappings.is_empty()).then(|| {
    SourceMap::new(
      None,
      Mappings::new(mappings, options),
      sources,
      sources_content,
      names,
    )
  })
}

pub trait StreamChunks {
  fn stream_chunks(
    &self,
    options: &MapOptions,
    on_chunk: OnChunk,
    on_source: OnSource,
    on_name: OnName,
  ) -> GeneratedInfo;
}

pub type OnChunk<'a> = &'a mut dyn FnMut(Mapping);
pub type OnSource<'a> = &'a mut dyn FnMut(u32, &str, Option<&str>);
pub type OnName<'a> = &'a mut dyn FnMut(u32, &str);

pub struct GeneratedInfo {
  pub generated_line: u32,
  pub generated_column: u32,
}

pub fn create_mapping_serializer(
  options: &MapOptions,
) -> impl MappingSerializer {
  if options.columns {
    Either::Left(FullMappingsSerializer::default())
  } else {
    Either::Right(LinesOnlyMappingsSerializer::default())
  }
}

pub enum Either<A, B> {
  Left(A),
  Right(B),
}

impl<A, B> MappingSerializer for Either<A, B>
where
  A: MappingSerializer,
  B: MappingSerializer,
{
  fn serialize(&mut self, mapping: &Mapping) -> Option<String> {
    match self {
      Self::Left(left) => left.serialize(mapping),
      Self::Right(right) => right.serialize(mapping),
    }
  }
}

pub trait MappingSerializer {
  fn serialize(&mut self, mapping: &Mapping) -> Option<String>;
}

pub struct FullMappingsSerializer {
  current_line: u32,
  current_column: u32,
  current_original_line: u32,
  current_original_column: u32,
  current_source_index: u32,
  current_name_index: u32,
  active_mapping: bool,
  active_name: bool,
  initial: bool,
}

impl Default for FullMappingsSerializer {
  fn default() -> Self {
    Self {
      current_line: 1,
      current_column: 0,
      current_original_line: 1,
      current_original_column: 0,
      current_source_index: 0,
      current_name_index: 0,
      active_mapping: false,
      active_name: false,
      initial: true,
    }
  }
}

impl MappingSerializer for FullMappingsSerializer {
  fn serialize(&mut self, mapping: &Mapping) -> Option<String> {
    if self.active_mapping && self.current_line == mapping.generated_line {
      // A mapping is still active
      if let Some(original) = &mapping.original
        && original.source_index == self.current_source_index
        && original.original_line == self.current_original_line
        && original.original_column == self.current_original_column
        && !self.active_name
        && original.name_index.is_none()
      {
        // avoid repeating the same original mapping
        return None;
      }
    } else {
      // No mapping is active
      if mapping.original.is_none() {
        // avoid writing unneccessary generated mappings
        return None;
      }
    }

    let mut out = String::new();
    if self.current_line < mapping.generated_line {
      (0..mapping.generated_line - self.current_line)
        .for_each(|_| out.push(';'));
      self.current_line = mapping.generated_line;
      self.current_column = 0;
      self.initial = false;
    } else if self.initial {
      self.initial = false;
    } else {
      out.push(',');
    }

    encode(&mut out, mapping.generated_column, self.current_column);
    self.current_column = mapping.generated_column;
    if let Some(original) = &mapping.original {
      self.active_mapping = true;
      if original.source_index == self.current_source_index {
        out.push('A');
      } else {
        encode(&mut out, original.source_index, self.current_source_index);
        self.current_source_index = original.source_index;
      }
      encode(&mut out, original.original_line, self.current_original_line);
      self.current_original_line = original.original_line;
      if original.original_column == self.current_original_column {
        out.push('A');
      } else {
        encode(
          &mut out,
          original.original_column,
          self.current_original_column,
        );
        self.current_original_column = original.original_column;
      }
      if let Some(name_index) = original.name_index {
        encode(&mut out, name_index, self.current_name_index);
        self.current_name_index = name_index;
        self.active_name = true;
      } else {
        self.active_name = false;
      }
    } else {
      self.active_mapping = false;
    }
    Some(out)
  }
}

pub struct LinesOnlyMappingsSerializer {
  last_written_line: u32,
  current_line: u32,
  current_source_index: u32,
  current_original_line: u32,
}

impl Default for LinesOnlyMappingsSerializer {
  fn default() -> Self {
    Self {
      last_written_line: 0,
      current_line: 1,
      current_source_index: 0,
      current_original_line: 1,
    }
  }
}

impl MappingSerializer for LinesOnlyMappingsSerializer {
  fn serialize(&mut self, mapping: &Mapping) -> Option<String> {
    if let Some(original) = &mapping.original {
      if self.last_written_line == mapping.generated_line {
        // avoid writing multiple original mappings per line
        return None;
      }
      let mut out = String::new();
      self.last_written_line = mapping.generated_line;
      if mapping.generated_line == self.current_line + 1 {
        self.current_line = mapping.generated_line;
        if original.source_index == self.current_source_index {
          if original.original_line == self.current_original_line + 1 {
            self.current_original_line = original.original_line;
            out.push_str(";AACA");
            return Some(out);
          } else {
            out.push_str(";AA");
            encode(
              &mut out,
              original.original_line,
              self.current_original_line,
            );
            self.current_original_line = original.original_line;
            out.push('A');
            return Some(out);
          }
        } else {
          out.push_str(";A");
          encode(&mut out, original.source_index, self.current_source_index);
          self.current_source_index = original.source_index;
          encode(&mut out, original.original_line, self.current_original_line);
          self.current_original_line = original.original_line;
          out.push('A');
          return Some(out);
        }
      } else {
        (0..mapping.generated_line - self.current_line)
          .for_each(|_| out.push(';'));
        self.current_line = mapping.generated_line;
        if original.source_index == self.current_source_index {
          if original.original_line == self.current_original_line + 1 {
            self.current_original_line = original.original_line;
            out.push_str("AACA");
            return Some(out);
          } else {
            out.push_str("AA");
            encode(
              &mut out,
              original.original_line,
              self.current_original_line,
            );
            self.current_original_line = original.original_line;
            out.push('A');
            return Some(out);
          }
        } else {
          out.push('A');
          encode(&mut out, original.source_index, self.current_source_index);
          self.current_source_index = original.source_index;
          encode(&mut out, original.original_line, self.current_original_line);
          self.current_original_line = original.original_line;
          out.push('A');
          return Some(out);
        }
      }
    }
    // avoid writing generated mappings at all
    None
  }
}

pub struct PotentialTokens<'a> {
  bytes: &'a [u8],
  source: &'a str,
  index: usize,
}

impl<'a> Iterator for PotentialTokens<'a> {
  type Item = &'a str;

  fn next(&mut self) -> Option<Self::Item> {
    if let Some(&c) = self.bytes.get(self.index) {
      let start = self.index;
      let mut c = char::from(c);
      while c != '\n' && c != ';' && c != '{' && c != '}' {
        self.index += 1;
        if let Some(&ch) = self.bytes.get(self.index) {
          c = char::from(ch);
        } else {
          return Some(&self.source[start..self.index]);
        }
      }
      while c == ';'
        || c == ' '
        || c == '{'
        || c == '}'
        || c == '\r'
        || c == '\t'
      {
        self.index += 1;
        if let Some(&ch) = self.bytes.get(self.index) {
          c = char::from(ch);
        } else {
          return Some(&self.source[start..self.index]);
        }
      }
      if c == '\n' {
        self.index += 1;
      }
      Some(&self.source[start..self.index])
    } else {
      None
    }
  }
}

// /[^\n;{}]+[;{} \r\t]*\n?|[;{} \r\t]+\n?|\n/g
pub fn split_into_potential_tokens(source: &str) -> PotentialTokens {
  PotentialTokens {
    bytes: source.as_bytes(),
    source,
    index: 0,
  }
}

pub struct PotentialLines<'a, I> {
  lines: I,
  index: usize,
  source: &'a str,
}

impl<'a, I> Iterator for PotentialLines<'a, I>
where
  I: Iterator<Item = (usize, char)>,
{
  type Item = &'a str;

  fn next(&mut self) -> Option<Self::Item> {
    self.lines.next().map(|(j, _)| {
      let i = self.index;
      self.index = j;
      &self.source[i..j + 1]
    })
  }
}

// /[^\n]+\n?|\n/g
pub fn split_into_potential_lines(
  source: &str,
) -> PotentialLines<impl Iterator<Item = (usize, char)> + '_> {
  PotentialLines {
    lines: source.char_indices().take_while(|(_, ch)| ch == &'\n'),
    index: 0,
    source,
  }
}

pub fn get_generated_source_info(source: &str) -> GeneratedInfo {
  let last_line_start = source.rfind('\n');
  if let Some(last_line_start) = last_line_start {
    let mut generated_line = 2;
    source[0..last_line_start].chars().for_each(|c| {
      if c == '\n' {
        generated_line += 1;
      }
    });
    return GeneratedInfo {
      generated_line,
      generated_column: (source.len() - last_line_start - 1) as u32,
    };
  }
  GeneratedInfo {
    generated_line: 1,
    generated_column: source.len() as u32, // TODO?
  }
}
