use crate::{
  source::{Mapping, OriginalLocation},
  vlq::{decode, encode},
  MapOptions, SourceMap,
};

pub fn get_map<S: StreamChunks>(
  stream: &S,
  options: &MapOptions,
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
    &mut |chunk, mapping| {
      mappings.push(mapping);
    },
    &mut |source_index, source: &str, source_content: Option<&str>| {
      let source_index = source_index as usize;
      while sources.len() <= source_index {
        sources.push("".to_string());
      }
      sources[source_index] = source.to_owned();
      if let Some(source_content) = source_content {
        while sources_content.len() <= source_index {
          sources_content.push("".to_string());
        }
        sources_content[source_index] = source_content.to_owned();
      }
    },
    &mut |name_index, name: &str| {
      let name_index = name_index as usize;
      while names.len() <= name_index {
        names.push("".to_string());
      }
      names[name_index] = name.to_owned();
    },
  );
  let mappings = encode_mappings(&mappings, options);
  (!mappings.is_empty())
    .then(|| SourceMap::new(None, mappings, sources, sources_content, names))
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

pub type OnChunk<'a> = &'a mut dyn FnMut(Option<&str>, Mapping);
pub type OnSource<'a> = &'a mut dyn FnMut(u32, &str, Option<&str>);
pub type OnName<'a> = &'a mut dyn FnMut(u32, &str);

pub struct GeneratedInfo {
  pub generated_line: u32,
  pub generated_column: u32,
}

pub fn decode_mappings(raw: &str) -> Vec<Mapping> {
  let mut generated_column = 0;
  let mut source_index = 0;
  let mut original_line = 1;
  let mut original_column = 0;
  let mut name_index = 0;
  let mut nums = Vec::with_capacity(6);

  let mut mappings = Vec::new();
  for (generated_line, line) in raw.split(';').enumerate() {
    if line.is_empty() {
      continue;
    }

    generated_column = 0;

    for segment in line.split(',') {
      if segment.is_empty() {
        continue;
      }

      nums.clear();
      decode(segment, &mut nums).unwrap();
      generated_column = (i64::from(generated_column) + nums[0]) as u32;

      let mut src = None;
      let mut name = None;

      if nums.len() > 1 {
        if nums.len() != 4 && nums.len() != 5 {
          panic!();
        }
        source_index = (i64::from(source_index) + nums[1]) as u32;

        src = Some(source_index);
        original_line = (i64::from(original_line) + nums[2]) as u32;
        original_column = (i64::from(original_column) + nums[3]) as u32;

        if nums.len() > 4 {
          name_index = (i64::from(name_index) + nums[4]) as u32;
          name = Some(name_index as u32);
        }
      }

      mappings.push(Mapping {
        generated_line: 1 + generated_line as u32,
        generated_column,
        original: src.map(|src_id| OriginalLocation {
          source_index: src_id,
          original_line,
          original_column,
          name_index: name,
        }),
      })
    }
  }
  mappings
}

pub fn encode_mappings(mappings: &[Mapping], options: &MapOptions) -> String {
  if options.columns {
    encode_full_mappings(mappings)
  } else {
    encode_lines_only_mappings(mappings)
  }
}

fn encode_full_mappings(mappings: &[Mapping]) -> String {
  let mut current_line = 1;
  let mut current_column = 0;
  let mut current_original_line = 1;
  let mut current_original_column = 0;
  let mut current_source_index = 0;
  let mut current_name_index = 0;
  let mut active_mapping = false;
  let mut active_name = false;
  let mut initial = true;

  mappings.iter().fold(String::new(), |acc, mapping| {
    if active_mapping && current_line == mapping.generated_line {
      // A mapping is still active
      if let Some(original) = &mapping.original
      && original.source_index == current_source_index
      && original.original_line == current_original_line
      && original.original_column == current_original_column
      && !active_name
      && original.name_index.is_none()
    {
      // avoid repeating the same original mapping
      return acc;
    }
    } else {
      // No mapping is active
      if mapping.original.is_none() {
        // avoid writing unneccessary generated mappings
        return acc;
      }
    }

    let mut out = String::new();
    if current_line < mapping.generated_line {
      (0..mapping.generated_line - current_line).for_each(|_| out.push(';'));
      current_line = mapping.generated_line;
      current_column = 0;
      initial = false;
    } else if initial {
      initial = false;
    } else {
      out.push(',');
    }

    encode(&mut out, mapping.generated_column, current_column);
    current_column = mapping.generated_column;
    if let Some(original) = &mapping.original {
      active_mapping = true;
      if original.source_index == current_source_index {
        out.push('A');
      } else {
        encode(&mut out, original.source_index, current_source_index);
        current_source_index = original.source_index;
      }
      encode(&mut out, original.original_line, current_original_line);
      current_original_line = original.original_line;
      if original.original_column == current_original_column {
        out.push('A');
      } else {
        encode(&mut out, original.original_column, current_original_column);
        current_original_column = original.original_column;
      }
      if let Some(name_index) = original.name_index {
        encode(&mut out, name_index, current_name_index);
        current_name_index = name_index;
        active_name = true;
      } else {
        active_name = false;
      }
    } else {
      active_mapping = false;
    }
    acc + &out
  })
}

fn encode_lines_only_mappings(mappings: &[Mapping]) -> String {
  let mut last_written_line = 0;
  let mut current_line = 1;
  let mut current_source_index = 0;
  let mut current_original_line = 1;
  mappings.iter().fold(String::new(), |acc, mapping| {
    if let Some(original) = &mapping.original {
      if last_written_line == mapping.generated_line {
        // avoid writing multiple original mappings per line
        return acc;
      }
      let mut out = String::new();
      last_written_line = mapping.generated_line;
      if mapping.generated_line == current_line + 1 {
        current_line = mapping.generated_line;
        if original.source_index == current_source_index {
          if original.original_line == current_original_line + 1 {
            current_original_line = original.original_line;
            out.push_str(";AACA");
            return acc + &out;
          } else {
            out.push_str(";AA");
            encode(&mut out, original.original_line, current_original_line);
            current_original_line = original.original_line;
            out.push('A');
            return acc + &out;
          }
        } else {
          out.push_str(";A");
          encode(&mut out, original.source_index, current_source_index);
          current_source_index = original.source_index;
          encode(&mut out, original.original_line, current_original_line);
          current_original_line = original.original_line;
          out.push('A');
          return acc + &out;
        }
      } else {
        (0..mapping.generated_line - current_line).for_each(|_| out.push(';'));
        current_line = mapping.generated_line;
        if original.source_index == current_source_index {
          if original.original_line == current_original_line + 1 {
            current_original_line = original.original_line;
            out.push_str("AACA");
            return acc + &out;
          } else {
            out.push_str("AA");
            encode(&mut out, original.original_line, current_original_line);
            current_original_line = original.original_line;
            out.push('A');
            return acc + &out;
          }
        } else {
          out.push('A');
          encode(&mut out, original.source_index, current_source_index);
          current_source_index = original.source_index;
          encode(&mut out, original.original_line, current_original_line);
          current_original_line = original.original_line;
          out.push('A');
          return acc + &out;
        }
      }
    }
    // avoid writing generated mappings at all
    acc
  })
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
pub fn split_into_lines(
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
    generated_column: source.len() as u32,
  }
}

pub fn stream_chunks_of_source_map(
  source: &str,
  source_map: &SourceMap,
  on_chunk: OnChunk,
  on_source: OnSource,
  on_name: OnName,
  options: &MapOptions,
) -> GeneratedInfo {
  match options {
    MapOptions {
      columns: true,
      final_source: true,
    } => stream_chunks_of_source_map_final(
      source, source_map, on_chunk, on_source, on_name,
    ),
    MapOptions {
      columns: false,
      final_source: true,
    } => stream_chunks_of_source_map_full(
      source, source_map, on_chunk, on_source, on_name,
    ),
    MapOptions {
      columns: true,
      final_source: false,
    } => stream_chunks_of_source_map_lines_final(
      source, source_map, on_chunk, on_source, on_name,
    ),
    MapOptions {
      columns: false,
      final_source: false,
    } => stream_chunks_of_source_map_lines_full(
      source, source_map, on_chunk, on_source, on_name,
    ),
  }
}

fn stream_chunks_of_source_map_final(
  source: &str,
  source_map: &SourceMap,
  on_chunk: OnChunk,
  on_source: OnSource,
  on_name: OnName,
) -> GeneratedInfo {
  let result = get_generated_source_info(source);
  if result.generated_line == 1 && result.generated_column == 0 {
    return result;
  }
  for (i, source) in source_map.sources().iter().enumerate() {
    on_source(i as u32, source, source_map.get_source_content(i))
  }
  for (i, name) in source_map.names().iter().enumerate() {
    on_name(i as u32, name);
  }
  let mut mapping_active_line = 0;
  let mut on_mapping = |mapping: &Mapping| {
    if mapping.generated_line >= result.generated_line
      && (mapping.generated_column >= result.generated_column
        || mapping.generated_line > result.generated_line)
    {
      return;
    }
    if let Some(original) = &mapping.original {
      on_chunk(
        None,
        Mapping {
          generated_line: mapping.generated_line,
          generated_column: mapping.generated_column,
          original: Some(original.clone()),
        },
      );
      mapping_active_line = mapping.generated_line;
    } else if mapping_active_line == mapping.generated_line {
      on_chunk(
        None,
        Mapping {
          generated_line: mapping.generated_line,
          generated_column: mapping.generated_column,
          original: None,
        },
      );
    }
  };
  for mapping in &source_map.decoded_mappings() {
    on_mapping(mapping);
  }
  result
}

fn stream_chunks_of_source_map_full(
  source: &str,
  source_map: &SourceMap,
  on_chunk: OnChunk,
  on_source: OnSource,
  on_name: OnName,
) -> GeneratedInfo {
  let lines: Vec<_> = split_into_lines(source).collect();
  if lines.is_empty() {
    return GeneratedInfo {
      generated_line: 1,
      generated_column: 0,
    };
  }
  for (i, source) in source_map.sources().iter().enumerate() {
    on_source(i as u32, source, source_map.get_source_content(i))
  }
  for (i, name) in source_map.names().iter().enumerate() {
    on_name(i as u32, name);
  }
  let last_line = lines[lines.len() - 1];
  let last_new_line = last_line.ends_with('\n');
  let final_line: u32 = if last_new_line {
    lines.len() + 1
  } else {
    lines.len()
  } as u32;
  let final_column: u32 =
    if last_new_line { 0 } else { last_line.len() } as u32;
  let mut current_generated_line: u32 = 1;
  let mut current_generated_column: u32 = 0;
  let mut mapping_active = false;
  let mut active_mapping_original: Option<OriginalLocation> = None;

  let mut on_mapping = |mapping: &Mapping| {
    if mapping_active && current_generated_column as usize <= lines.len() {
      let chunk;
      let mapping_line = current_generated_line;
      let mapping_column = current_generated_column;
      let line = lines[(current_generated_line - 1) as usize];
      if mapping.generated_line != current_generated_line {
        chunk = &line[current_generated_column as usize..];
        current_generated_line += 1;
        current_generated_column = 0;
      } else {
        chunk = &line[current_generated_column as usize
          ..mapping.generated_column as usize];
        current_generated_column = mapping.generated_column;
      }
      if !chunk.is_empty() {
        on_chunk(
          Some(chunk),
          Mapping {
            generated_line: mapping_line,
            generated_column: mapping_column,
            original: active_mapping_original.clone(),
          },
        )
      }
      mapping_active = false;
    }
    if mapping.generated_line > current_generated_line
      && current_generated_column > 0
    {
      if current_generated_line as usize <= lines.len() {
        let chunk = &lines[(current_generated_line - 1) as usize]
          [current_generated_column as usize..];
        on_chunk(
          Some(chunk),
          Mapping {
            generated_line: current_generated_line,
            generated_column: current_generated_column,
            original: None,
          },
        );
      }
      current_generated_line += 1;
      current_generated_column = 0;
    }
    while mapping.generated_line > current_generated_line {
      if current_generated_line as usize <= lines.len() {
        on_chunk(
          Some(lines[(current_generated_line as usize) - 1]),
          Mapping {
            generated_line: current_generated_line,
            generated_column: 0,
            original: None,
          },
        );
      }
      current_generated_line += 1;
    }
    if mapping.generated_column > current_generated_column {
      if current_generated_line as usize <= lines.len() {
        let chunk = &lines[(current_generated_line as usize) - 1]
          [current_generated_column as usize
            ..mapping.generated_column as usize];
        on_chunk(
          Some(chunk),
          Mapping {
            generated_line: current_generated_line,
            generated_column: current_generated_column,
            original: None,
          },
        )
      }
      current_generated_column = mapping.generated_column;
    }
    if let Some(original) = &mapping.original
      && (mapping.generated_line < final_line
        || (mapping.generated_line == final_line
        && mapping.generated_column < final_column)) {
      mapping_active = true;
      active_mapping_original = Some(original.clone());
    }
  };

  for mapping in &source_map.decoded_mappings() {
    on_mapping(mapping);
  }
  on_mapping(&Mapping {
    generated_line: final_line,
    generated_column: final_column,
    original: None,
  });
  GeneratedInfo {
    generated_line: final_line,
    generated_column: final_column,
  }
}

fn stream_chunks_of_source_map_lines_final(
  source: &str,
  source_map: &SourceMap,
  on_chunk: OnChunk,
  on_source: OnSource,
  _on_name: OnName,
) -> GeneratedInfo {
  let result = get_generated_source_info(source);
  if result.generated_line == 1 && result.generated_column == 0 {
    return GeneratedInfo {
      generated_line: 1,
      generated_column: 0,
    };
  }
  for (i, source) in source_map.sources().iter().enumerate() {
    on_source(i as u32, source, source_map.get_source_content(i))
  }
  let final_line = if result.generated_column == 0 {
    result.generated_line - 1
  } else {
    result.generated_line
  };
  let mut current_generated_line = 1;

  let mut on_mapping = |mapping: &Mapping| {
    if let Some(original) = &mapping.original
      && current_generated_line <= result.generated_line
      && result.generated_line <= final_line {
      on_chunk(None, Mapping {
        generated_line: result.generated_line,
        generated_column: 0,
        original: Some(OriginalLocation {
          source_index: original.source_index,
          original_line: original.original_line,
          original_column: original.original_column,
          name_index: None,
        }),
      });
      current_generated_line = result.generated_line + 1;
    }
  };
  for mapping in &source_map.decoded_mappings() {
    on_mapping(mapping);
  }
  result
}

fn stream_chunks_of_source_map_lines_full(
  source: &str,
  source_map: &SourceMap,
  on_chunk: OnChunk,
  on_source: OnSource,
  _on_name: OnName,
) -> GeneratedInfo {
  let lines: Vec<&str> = split_into_lines(source).collect();
  if lines.is_empty() {
    return GeneratedInfo {
      generated_line: 1,
      generated_column: 0,
    };
  }
  for (i, source) in source_map.sources().iter().enumerate() {
    on_source(i as u32, source, source_map.get_source_content(i))
  }
  let mut current_generated_line = 1;
  let mut on_mapping = |mapping: &Mapping| {
    if mapping.original.is_none()
      && mapping.generated_line < current_generated_line
      && mapping.generated_line as usize > lines.len()
    {
      return;
    }
    while mapping.generated_line > current_generated_line {
      if current_generated_line as usize <= lines.len() {
        on_chunk(
          Some(lines[current_generated_line as usize - 1]),
          Mapping {
            generated_line: current_generated_line,
            generated_column: 0,
            original: None,
          },
        );
      }
      current_generated_line += 1;
    }
    if let Some(original) = &mapping.original && mapping.generated_line as usize <= lines.len() {
      on_chunk(Some(lines[mapping.generated_line as usize - 1]), Mapping {
        generated_line: mapping.generated_line,
        generated_column: 0,
        original: Some(OriginalLocation {
          source_index: original.source_index,
          original_line: original.original_line,
          original_column: original.original_column,
          name_index: None,
        }),
      });
      current_generated_line += 1;
    }
  };
  for mapping in &source_map.decoded_mappings() {
    on_mapping(mapping);
  }
  while current_generated_line as usize <= lines.len() {
    on_chunk(
      Some(lines[current_generated_line as usize - 1]),
      Mapping {
        generated_line: current_generated_line,
        generated_column: 0,
        original: None,
      },
    );
  }
  let last_line = lines[lines.len() - 1];
  let last_new_line = last_line.ends_with("\n");
  let final_line = if last_new_line {
    lines.len() + 1
  } else {
    lines.len()
  } as u32;
  let final_column = if last_new_line { 0 } else { last_line.len() } as u32;
  GeneratedInfo {
    generated_line: final_line,
    generated_column: final_column,
  }
}
