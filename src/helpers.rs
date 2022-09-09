use crate::{
  source::{Mapping, Mappings, OriginalLocation},
  vlq::{decode, encode},
  Error, MapOptions, Result, SourceMap,
};

pub fn get_map<S: StreamChunks>(stream: &S, options: MapOptions) -> SourceMap {
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
    &mut |source_index, source: Option<&str>, source_content: Option<&str>| {
      let source_index = source_index as usize;
      while sources.len() <= source_index {
        sources.push(None);
      }
      sources[source_index] = source.map(ToOwned::to_owned);
      if let Some(source_content) = source_content {
        while sources_content.len() <= source_index {
          sources_content.push(None);
        }
        sources_content[source_index] = Some(source_content.to_string());
      }
    },
    &mut |name_index, name: Option<&str>| {
      let name_index = name_index as usize;
      while names.len() <= name_index {
        names.push(None);
      }
      names[name_index] = name.map(ToOwned::to_owned);
    },
  );
  SourceMap::new(
    None,
    Mappings::new(mappings, options),
    None,
    sources,
    sources_content,
    names,
  )
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
pub type OnSource<'a> = &'a mut dyn FnMut(u32, Option<&str>, Option<&str>);
pub type OnName<'a> = &'a mut dyn FnMut(u32, Option<&str>);

pub struct GeneratedInfo {
  pub generated_line: u32,
  pub generated_column: u32,
}

pub trait MappingsDeserializer {
  fn deserialize(&mut self, serialized: &str) -> Result<Vec<Mapping>>;
}

pub struct NormalMappingsDeserializer {
  generated_column: u32,
  source_index: u32,
  original_line: u32,
  original_column: u32,
  name_index: u32,
  nums: Vec<i64>,
}

impl Default for NormalMappingsDeserializer {
  fn default() -> Self {
    Self {
      generated_column: 0,
      source_index: 0,
      original_line: 0,
      original_column: 0,
      name_index: 0,
      nums: Vec::with_capacity(6),
    }
  }
}

impl MappingsDeserializer for NormalMappingsDeserializer {
  fn deserialize(&mut self, serialized: &str) -> Result<Vec<Mapping>> {
    let mut mappings = Vec::new();
    for (generated_line, line) in serialized.split(';').enumerate() {
      if line.is_empty() {
        continue;
      }

      self.generated_column = 0;

      for segment in line.split(',') {
        if segment.is_empty() {
          continue;
        }

        self.nums.clear();
        decode(segment, &mut self.nums)?;
        self.generated_column =
          (i64::from(self.generated_column) + self.nums[0]) as u32;

        let mut src = None;
        let mut name = None;

        if self.nums.len() > 1 {
          if self.nums.len() != 4 && self.nums.len() != 5 {
            return Err(Error::BadSegmentSize(self.nums.len() as u32));
          }
          self.source_index =
            (i64::from(self.source_index) + self.nums[1]) as u32;

          src = Some(self.source_index);
          self.original_line =
            (i64::from(self.original_line) + self.nums[2]) as u32;
          self.original_column =
            (i64::from(self.original_column) + self.nums[3]) as u32;

          if self.nums.len() > 4 {
            self.name_index =
              (i64::from(self.name_index) + self.nums[4]) as u32;
            name = Some(self.name_index as u32);
          }
        }

        mappings.push(Mapping {
          generated_line: generated_line as u32,
          generated_column: self.generated_column,
          original: src.map(|src_id| OriginalLocation {
            source_index: src_id,
            original_line: self.original_line,
            original_column: self.original_column,
            name_index: name,
          }),
        })
      }
    }
    Ok(mappings)
  }
}

pub fn create_mappings_serializer(
  options: &MapOptions,
) -> impl MappingsSerializer {
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

impl<A, B> MappingsSerializer for Either<A, B>
where
  A: MappingsSerializer,
  B: MappingsSerializer,
{
  fn serialize(&mut self, mappings: &[Mapping]) -> String {
    match self {
      Self::Left(left) => left.serialize(mappings),
      Self::Right(right) => right.serialize(mappings),
    }
  }
}

pub trait MappingsSerializer {
  fn serialize(&mut self, mappings: &[Mapping]) -> String;
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

impl MappingsSerializer for FullMappingsSerializer {
  fn serialize(&mut self, mappings: &[Mapping]) -> String {
    mappings.iter().fold(String::new(), |acc, mapping| {
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
      acc + &out
    })
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

impl MappingsSerializer for LinesOnlyMappingsSerializer {
  fn serialize(&mut self, mappings: &[Mapping]) -> String {
    mappings.iter().fold(String::new(), |acc, mapping| {
      if let Some(original) = &mapping.original {
        if self.last_written_line == mapping.generated_line {
          // avoid writing multiple original mappings per line
          return acc;
        }
        let mut out = String::new();
        self.last_written_line = mapping.generated_line;
        if mapping.generated_line == self.current_line + 1 {
          self.current_line = mapping.generated_line;
          if original.source_index == self.current_source_index {
            if original.original_line == self.current_original_line + 1 {
              self.current_original_line = original.original_line;
              out.push_str(";AACA");
              return acc + &out;
            } else {
              out.push_str(";AA");
              encode(
                &mut out,
                original.original_line,
                self.current_original_line,
              );
              self.current_original_line = original.original_line;
              out.push('A');
              return acc + &out;
            }
          } else {
            out.push_str(";A");
            encode(&mut out, original.source_index, self.current_source_index);
            self.current_source_index = original.source_index;
            encode(
              &mut out,
              original.original_line,
              self.current_original_line,
            );
            self.current_original_line = original.original_line;
            out.push('A');
            return acc + &out;
          }
        } else {
          (0..mapping.generated_line - self.current_line)
            .for_each(|_| out.push(';'));
          self.current_line = mapping.generated_line;
          if original.source_index == self.current_source_index {
            if original.original_line == self.current_original_line + 1 {
              self.current_original_line = original.original_line;
              out.push_str("AACA");
              return acc + &out;
            } else {
              out.push_str("AA");
              encode(
                &mut out,
                original.original_line,
                self.current_original_line,
              );
              self.current_original_line = original.original_line;
              out.push('A');
              return acc + &out;
            }
          } else {
            out.push('A');
            encode(&mut out, original.source_index, self.current_source_index);
            self.current_source_index = original.source_index;
            encode(
              &mut out,
              original.original_line,
              self.current_original_line,
            );
            self.current_original_line = original.original_line;
            out.push('A');
            return acc + &out;
          }
        }
      }
      // avoid writing generated mappings at all
      acc
    })
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
  for i in 0..source_map.sources().count() {
    on_source(
      i as u32,
      source_map.get_source(i).as_deref(),
      source_map.sources_content().nth(i).and_then(|c| c),
    );
  }
  for (i, name) in source_map.names().enumerate() {
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
      on_chunk(Mapping {
        generated_line: mapping.generated_line,
        generated_column: mapping.generated_column,
        original: Some(original.clone()),
      });
      mapping_active_line = mapping.generated_line;
    } else if mapping_active_line == mapping.generated_line {
      on_chunk(Mapping {
        generated_line: mapping.generated_line,
        generated_column: mapping.generated_column,
        original: None,
      });
    }
  };
  for mapping in source_map.mappings().iter() {
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
  let lines: Vec<_> = split_into_potential_lines(source).collect();
  if lines.is_empty() {
    return GeneratedInfo {
      generated_line: 1,
      generated_column: 0,
    };
  }
  for i in 0..source_map.sources().count() {
    on_source(
      i as u32,
      source_map.get_source(i).as_deref(),
      source_map.sources_content().nth(i).and_then(|c| c),
    )
  }
  for (i, name) in source_map.names().enumerate() {
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
        on_chunk(Mapping {
          generated_line: mapping_line,
          generated_column: mapping_column,
          original: active_mapping_original.clone(),
        })
      }
      mapping_active = false;
    }
    if mapping.generated_line > current_generated_line
      && current_generated_column > 0
    {
      if current_generated_line as usize <= lines.len() {
        on_chunk(Mapping {
          generated_line: current_generated_line,
          generated_column: current_generated_column,
          original: None,
        });
      }
      current_generated_line += 1;
      current_generated_column = 0;
    }
    while mapping.generated_line > current_generated_line {
      if current_generated_line as usize <= lines.len() {
        on_chunk(Mapping {
          generated_line: current_generated_line,
          generated_column: 0,
          original: None,
        });
      }
      current_generated_line += 1;
    }
    if mapping.generated_column > current_generated_column {
      if current_generated_line as usize <= lines.len() {
        on_chunk(Mapping {
          generated_line: current_generated_line,
          generated_column: current_generated_column,
          original: None,
        })
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

  for mapping in source_map.mappings().iter() {
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
  on_name: OnName,
) -> GeneratedInfo {
  todo!()
}

fn stream_chunks_of_source_map_lines_full(
  source: &str,
  source_map: &SourceMap,
  on_chunk: OnChunk,
  on_source: OnSource,
  on_name: OnName,
) -> GeneratedInfo {
  todo!()
}
