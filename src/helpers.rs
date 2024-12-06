use std::{
  borrow::{BorrowMut, Cow},
  cell::{OnceCell, RefCell},
};

use rustc_hash::FxHashMap as HashMap;

use crate::{
  decoder::MappingsDecoder,
  encoder::create_encoder,
  linear_map::LinearMap,
  source::{Mapping, OriginalLocation},
  with_indices::WithIndices,
  MapOptions, SourceMap,
};

// Adding this type because sourceContentLine not happy
type InnerSourceContentLine<'a> =
  RefCell<LinearMap<OnceCell<Option<Vec<WithIndices<'a>>>>>>;

pub fn get_map<'a, S: StreamChunks<'a>>(
  stream: &'a S,
  options: &'a MapOptions,
) -> Option<SourceMap> {
  let mut mappings_encoder = create_encoder(options.columns);
  let mut sources: Vec<String> = Vec::new();
  let mut sources_content: Vec<String> = Vec::new();
  let mut names: Vec<String> = Vec::new();

  stream.stream_chunks(
    &MapOptions {
      columns: options.columns,
      final_source: true,
    },
    // on_chunk
    &mut |_, mapping| {
      mappings_encoder.encode(&mapping);
    },
    // on_source
    &mut |source_index, source, source_content| {
      let source_index = source_index as usize;
      if sources.len() <= source_index {
        sources.resize(source_index + 1, "".to_string());
      }
      sources[source_index] = source.to_string();
      if let Some(source_content) = source_content {
        if sources_content.len() <= source_index {
          sources_content.resize(source_index + 1, "".to_string());
        }
        sources_content[source_index] = source_content.to_string();
      }
    },
    // on_name
    &mut |name_index, name| {
      let name_index = name_index as usize;
      if names.len() <= name_index {
        names.resize(name_index + 1, "".to_string());
      }
      names[name_index] = name.to_string();
    },
  );
  let mappings = mappings_encoder.drain();
  (!mappings.is_empty())
    .then(|| SourceMap::new(mappings, sources, sources_content, names))
}

/// [StreamChunks] abstraction, see [webpack-sources source.streamChunks](https://github.com/webpack/webpack-sources/blob/9f98066311d53a153fdc7c633422a1d086528027/lib/helpers/streamChunks.js#L13).
pub trait StreamChunks<'a> {
  /// [StreamChunks] abstraction
  fn stream_chunks(
    &'a self,
    options: &MapOptions,
    on_chunk: OnChunk<'_, 'a>,
    on_source: OnSource<'_, 'a>,
    on_name: OnName<'_, 'a>,
  ) -> GeneratedInfo;
}

/// [OnChunk] abstraction, see [webpack-sources onChunk](https://github.com/webpack/webpack-sources/blob/9f98066311d53a153fdc7c633422a1d086528027/lib/helpers/streamChunks.js#L13).
pub type OnChunk<'a, 'b> = &'a mut dyn FnMut(Option<Cow<'b, str>>, Mapping);

/// [OnSource] abstraction, see [webpack-sources onSource](https://github.com/webpack/webpack-sources/blob/9f98066311d53a153fdc7c633422a1d086528027/lib/helpers/streamChunks.js#L13).
pub type OnSource<'a, 'b> =
  &'a mut dyn FnMut(u32, Cow<'b, str>, Option<&'b str>);

/// [OnName] abstraction, see [webpack-sources onName](https://github.com/webpack/webpack-sources/blob/9f98066311d53a153fdc7c633422a1d086528027/lib/helpers/streamChunks.js#L13).
pub type OnName<'a, 'b> = &'a mut dyn FnMut(u32, Cow<'b, str>);

/// Default stream chunks behavior impl, see [webpack-sources streamChunks](https://github.com/webpack/webpack-sources/blob/9f98066311d53a153fdc7c633422a1d086528027/lib/helpers/streamChunks.js#L15-L35).
pub fn stream_chunks_default<'a>(
  source: &'a str,
  source_map: Option<&'a SourceMap>,
  options: &MapOptions,
  on_chunk: OnChunk<'_, 'a>,
  on_source: OnSource<'_, 'a>,
  on_name: OnName<'_, 'a>,
) -> GeneratedInfo {
  if let Some(map) = source_map {
    stream_chunks_of_source_map(
      source, map, on_chunk, on_source, on_name, options,
    )
  } else {
    stream_chunks_of_raw_source(source, options, on_chunk, on_source, on_name)
  }
}

/// `GeneratedSourceInfo` abstraction, see [webpack-sources GeneratedSourceInfo](https://github.com/webpack/webpack-sources/blob/9f98066311d53a153fdc7c633422a1d086528027/lib/helpers/getGeneratedSourceInfo.js)
#[derive(Debug, PartialEq, Eq)]
pub struct GeneratedInfo {
  /// Generated line
  pub generated_line: u32,
  /// Generated column
  pub generated_column: u32,
}

/// Decodes the given mappings string into an iterator of `Mapping` items.
pub fn decode_mappings(
  source_map: &SourceMap,
) -> impl Iterator<Item = Mapping> + '_ {
  MappingsDecoder::new(source_map.mappings())
}

/// Encodes the given iterator of `Mapping` items into a `String`.
pub fn encode_mappings(mappings: impl Iterator<Item = Mapping>) -> String {
  let mut encoder = create_encoder(true);
  mappings.for_each(|mapping| encoder.encode(&mapping));
  encoder.drain()
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

/// Split the string with a needle, each string will contain the needle.
///
/// Copied and modified from https://github.com/rust-lang/cargo/blob/30efe860c0e4adc1a6d7057ad223dc6e47d34edf/src/cargo/sources/registry/index.rs#L1048-L1072
fn split(haystack: &str, needle: u8) -> impl Iterator<Item = &str> {
  struct Split<'a> {
    haystack: &'a str,
    needle: u8,
  }

  impl<'a> Iterator for Split<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
      if self.haystack.is_empty() {
        return None;
      }
      let (ret, remaining) =
        match memchr::memchr(self.needle, self.haystack.as_bytes()) {
          Some(pos) => (&self.haystack[..=pos], &self.haystack[pos + 1..]),
          None => (self.haystack, ""),
        };
      self.haystack = remaining;
      Some(ret)
    }
  }

  Split { haystack, needle }
}

// /[^\n]+\n?|\n/g
pub fn split_into_lines(source: &str) -> impl Iterator<Item = &str> {
  split(source, b'\n')
}

pub fn get_generated_source_info(source: &str) -> GeneratedInfo {
  let (generated_line, generated_column) = if source.ends_with('\n') {
    (split(source, b'\n').count() + 1, 0)
  } else {
    let mut line_count = 0;
    let mut last_line = "";
    for line in split(source, b'\n') {
      line_count += 1;
      last_line = line;
    }
    (line_count.max(1), last_line.len())
  };
  GeneratedInfo {
    generated_line: generated_line as u32,
    generated_column: generated_column as u32,
  }
}

pub fn stream_chunks_of_raw_source<'a>(
  source: &'a str,
  options: &MapOptions,
  on_chunk: OnChunk<'_, 'a>,
  _on_source: OnSource<'_, 'a>,
  _on_name: OnName<'_, 'a>,
) -> GeneratedInfo {
  if options.final_source {
    return get_generated_source_info(source);
  }

  let mut line = 1;
  let mut last_line = None;
  for l in split_into_lines(source) {
    on_chunk(
      Some(Cow::Borrowed(l)),
      Mapping {
        generated_line: line,
        generated_column: 0,
        original: None,
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

pub fn stream_chunks_of_source_map<'a>(
  source: &'a str,
  source_map: &'a SourceMap,
  on_chunk: OnChunk<'_, 'a>,
  on_source: OnSource<'_, 'a>,
  on_name: OnName<'_, 'a>,
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
      columns: true,
      final_source: false,
    } => stream_chunks_of_source_map_full(
      source, source_map, on_chunk, on_source, on_name,
    ),
    MapOptions {
      columns: false,
      final_source: true,
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

fn get_source<'a>(source_map: &SourceMap, source: &'a str) -> Cow<'a, str> {
  let source_root = source_map.source_root();
  match source_root {
    Some(root) if root.is_empty() => Cow::Borrowed(source),
    Some(root) if root.ends_with('/') => {
      Cow::Owned(format!("{}{}", root, source))
    }
    Some(root) => Cow::Owned(format!("{}/{}", root, source)),
    None => Cow::Borrowed(source),
  }
}

fn stream_chunks_of_source_map_final<'a>(
  source: &'a str,
  source_map: &'a SourceMap,
  on_chunk: OnChunk,
  on_source: OnSource<'_, 'a>,
  on_name: OnName<'_, 'a>,
) -> GeneratedInfo {
  let result = get_generated_source_info(source);
  if result.generated_line == 1 && result.generated_column == 0 {
    return result;
  }
  for (i, source) in source_map.sources().iter().enumerate() {
    on_source(
      i as u32,
      get_source(source_map, source),
      source_map.get_source_content(i),
    )
  }
  for (i, name) in source_map.names().iter().enumerate() {
    on_name(i as u32, Cow::Borrowed(name));
  }
  let mut mapping_active_line = 0;
  let mut on_mapping = |mapping: Mapping| {
    if mapping.generated_line >= result.generated_line
      && (mapping.generated_column >= result.generated_column
        || mapping.generated_line > result.generated_line)
    {
      return;
    }
    if let Some(original) = mapping.original {
      on_chunk(
        None,
        Mapping {
          generated_line: mapping.generated_line,
          generated_column: mapping.generated_column,
          original: Some(original),
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
  for mapping in source_map.decoded_mappings() {
    on_mapping(mapping);
  }
  result
}

fn stream_chunks_of_source_map_full<'a>(
  source: &'a str,
  source_map: &'a SourceMap,
  on_chunk: OnChunk<'_, 'a>,
  on_source: OnSource<'_, 'a>,
  on_name: OnName<'_, 'a>,
) -> GeneratedInfo {
  let lines = split_into_lines(source);
  let line_with_indices_list = lines.map(WithIndices::new).collect::<Vec<_>>();

  if line_with_indices_list.is_empty() {
    return GeneratedInfo {
      generated_line: 1,
      generated_column: 0,
    };
  }
  for (i, source) in source_map.sources().iter().enumerate() {
    on_source(
      i as u32,
      get_source(source_map, source),
      source_map.get_source_content(i),
    )
  }
  for (i, name) in source_map.names().iter().enumerate() {
    on_name(i as u32, Cow::Borrowed(name));
  }
  let last_line = line_with_indices_list[line_with_indices_list.len() - 1].line;
  let last_new_line = last_line.ends_with('\n');
  let final_line: u32 = if last_new_line {
    line_with_indices_list.len() + 1
  } else {
    line_with_indices_list.len()
  } as u32;
  let final_column: u32 =
    if last_new_line { 0 } else { last_line.len() } as u32;
  let mut current_generated_line: u32 = 1;
  let mut current_generated_column: u32 = 0;
  let mut mapping_active = false;
  let mut active_mapping_original: Option<OriginalLocation> = None;

  let mut on_mapping = |mapping: Mapping| {
    if mapping_active
      && current_generated_line as usize <= line_with_indices_list.len()
    {
      let chunk: &str;
      let mapping_line = current_generated_line;
      let mapping_column = current_generated_column;
      let line = &line_with_indices_list[(current_generated_line - 1) as usize];
      if mapping.generated_line != current_generated_line {
        chunk = line.substring(current_generated_column as usize, usize::MAX);
        current_generated_line += 1;
        current_generated_column = 0;
      } else {
        chunk = line.substring(
          current_generated_column as usize,
          mapping.generated_column as usize,
        );
        current_generated_column = mapping.generated_column;
      }
      if !chunk.is_empty() {
        on_chunk(
          Some(Cow::Borrowed(chunk)),
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
      if current_generated_line as usize <= line_with_indices_list.len() {
        let chunk = &line_with_indices_list
          [(current_generated_line - 1) as usize]
          .substring(current_generated_column as usize, usize::MAX);
        on_chunk(
          Some(Cow::Borrowed(chunk)),
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
      if current_generated_line as usize <= line_with_indices_list.len() {
        let chunk =
          line_with_indices_list[(current_generated_line as usize) - 1].line;
        on_chunk(
          Some(Cow::Borrowed(chunk)),
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
      if current_generated_line as usize <= line_with_indices_list.len() {
        let chunk = line_with_indices_list
          [(current_generated_line as usize) - 1]
          .substring(
            current_generated_column as usize,
            mapping.generated_column as usize,
          );
        on_chunk(
          Some(Cow::Borrowed(chunk)),
          Mapping {
            generated_line: current_generated_line,
            generated_column: current_generated_column,
            original: None,
          },
        )
      }
      current_generated_column = mapping.generated_column;
    }
    if let Some(original) = mapping.original.filter(|_| {
      mapping.generated_line < final_line
        || (mapping.generated_line == final_line
          && mapping.generated_column < final_column)
    }) {
      mapping_active = true;
      active_mapping_original = Some(original);
    }
  };

  for mapping in source_map.decoded_mappings() {
    on_mapping(mapping);
  }
  on_mapping(Mapping {
    generated_line: final_line,
    generated_column: final_column,
    original: None,
  });
  GeneratedInfo {
    generated_line: final_line,
    generated_column: final_column,
  }
}

fn stream_chunks_of_source_map_lines_final<'a>(
  source: &'a str,
  source_map: &'a SourceMap,
  on_chunk: OnChunk,
  on_source: OnSource<'_, 'a>,
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
    on_source(
      i as u32,
      get_source(source_map, source),
      source_map.get_source_content(i),
    )
  }
  let final_line = if result.generated_column == 0 {
    result.generated_line - 1
  } else {
    result.generated_line
  };
  let mut current_generated_line = 1;

  let mut on_mapping = |mut mapping: Mapping| {
    if let Some(original) = mapping.original.as_mut().filter(|_| {
      current_generated_line <= mapping.generated_line
        && mapping.generated_line <= final_line
    }) {
      mapping.generated_column = 0;
      original.name_index = None;
      current_generated_line = mapping.generated_line + 1;
      on_chunk(None, mapping);
    }
  };
  for mapping in source_map.decoded_mappings() {
    on_mapping(mapping);
  }
  result
}

fn stream_chunks_of_source_map_lines_full<'a>(
  source: &'a str,
  source_map: &'a SourceMap,
  on_chunk: OnChunk<'_, 'a>,
  on_source: OnSource<'_, 'a>,
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
    on_source(
      i as u32,
      get_source(source_map, source),
      source_map.get_source_content(i),
    )
  }
  let mut current_generated_line = 1;
  let mut on_mapping = |mut mapping: Mapping| {
    if mapping.original.is_none()
      || mapping.generated_line < current_generated_line
      || mapping.generated_line as usize > lines.len()
    {
      return;
    }
    while mapping.generated_line > current_generated_line {
      if current_generated_line as usize <= lines.len() {
        let chunk = lines[current_generated_line as usize - 1];
        on_chunk(
          Some(Cow::Borrowed(chunk)),
          Mapping {
            generated_line: current_generated_line,
            generated_column: 0,
            original: None,
          },
        );
      }
      current_generated_line += 1;
    }
    if let Some(original) = mapping
      .original
      .as_mut()
      .filter(|_| mapping.generated_line as usize <= lines.len())
    {
      let chunk = lines[current_generated_line as usize - 1];
      mapping.generated_column = 0;
      original.name_index = None;
      on_chunk(Some(Cow::Borrowed(chunk)), mapping);
      current_generated_line += 1;
    }
  };
  for mapping in source_map.decoded_mappings() {
    on_mapping(mapping);
  }
  while current_generated_line as usize <= lines.len() {
    let chunk = lines[current_generated_line as usize - 1];
    on_chunk(
      Some(Cow::Borrowed(chunk)),
      Mapping {
        generated_line: current_generated_line,
        generated_column: 0,
        original: None,
      },
    );
    current_generated_line += 1;
  }
  let last_line = lines[lines.len() - 1];
  let last_new_line = last_line.ends_with('\n');
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

#[derive(Debug)]
struct SourceMapLineData<'a> {
  pub mappings_data: Vec<i64>,
  pub chunks: Vec<Cow<'a, str>>,
}

type InnerSourceIndexValueMapping<'a> =
  LinearMap<(Cow<'a, str>, Option<&'a str>)>;

#[allow(clippy::too_many_arguments)]
pub fn stream_chunks_of_combined_source_map<'a>(
  source: &'a str,
  source_map: &'a SourceMap,
  inner_source_name: &'a str,
  inner_source: Option<&'a str>,
  inner_source_map: &'a SourceMap,
  remove_inner_source: bool,
  on_chunk: OnChunk<'_, 'a>,
  on_source: OnSource<'_, 'a>,
  on_name: OnName<'_, 'a>,
  options: &MapOptions,
) -> GeneratedInfo {
  let on_source = RefCell::new(on_source);
  let inner_source: RefCell<Option<&str>> = RefCell::new(inner_source);
  let source_mapping: RefCell<HashMap<Cow<str>, u32>> =
    RefCell::new(HashMap::default());
  let mut name_mapping: HashMap<Cow<str>, u32> = HashMap::default();
  let source_index_mapping: RefCell<LinearMap<i64>> =
    RefCell::new(LinearMap::default());
  let name_index_mapping: RefCell<LinearMap<i64>> =
    RefCell::new(LinearMap::default());
  let name_index_value_mapping: RefCell<LinearMap<Cow<str>>> =
    RefCell::new(LinearMap::default());
  let inner_source_index: RefCell<i64> = RefCell::new(-2);
  let inner_source_index_mapping: RefCell<LinearMap<i64>> =
    RefCell::new(LinearMap::default());
  let inner_source_index_value_mapping: RefCell<InnerSourceIndexValueMapping> =
    RefCell::new(LinearMap::default());
  let inner_source_contents: RefCell<LinearMap<Option<&str>>> =
    RefCell::new(LinearMap::default());
  let inner_source_content_lines: InnerSourceContentLine =
    RefCell::new(LinearMap::default());
  let inner_name_index_mapping: RefCell<LinearMap<i64>> =
    RefCell::new(LinearMap::default());
  let inner_name_index_value_mapping: RefCell<LinearMap<Cow<str>>> =
    RefCell::new(LinearMap::default());
  let inner_source_map_line_data: RefCell<Vec<SourceMapLineData>> =
    RefCell::new(Vec::new());

  let find_inner_mapping = |line: i64, column: i64| -> Option<u32> {
    let inner_source_map_line_data = inner_source_map_line_data.borrow();
    if line as usize > inner_source_map_line_data.len() {
      return None;
    }
    let mappings_data =
      &inner_source_map_line_data[line as usize - 1].mappings_data;
    let mut l = 0;
    let mut r = mappings_data.len() / 5;
    while l < r {
      let m = (l + r) >> 1;
      if mappings_data[m * 5] <= column {
        l = m + 1;
      } else {
        r = m;
      }
    }
    if l == 0 {
      return None;
    }
    Some(l as u32 - 1)
  };

  stream_chunks_of_source_map(
    source,
    source_map,
    &mut |chunk, mapping| {
      let source_index = mapping
        .original
        .as_ref()
        .map_or(-1, |o| o.source_index as i64);
      let original_line = mapping
        .original
        .as_ref()
        .map_or(-1, |o| o.original_line as i64);
      let original_column = mapping
        .original
        .as_ref()
        .map_or(-1, |o| o.original_column as i64);
      let name_index = mapping
        .original
        .as_ref()
        .and_then(|o| o.name_index)
        .map(|i| i as i64)
        .unwrap_or(-1);

      // Check if this is a mapping to the inner source
      if source_index == *inner_source_index.borrow() {
        let source_index = source_index as u32;

        // Check if there is a mapping in the inner source
        if let Some(idx) = find_inner_mapping(original_line, original_column) {
          let idx = idx as usize;
          let SourceMapLineData {
            mappings_data,
            chunks,
          } = &inner_source_map_line_data.borrow()[original_line as usize - 1];
          let mi = idx * 5;
          let inner_source_index = mappings_data[mi + 1];
          let inner_original_line = mappings_data[mi + 2];
          let mut inner_original_column = mappings_data[mi + 3];
          let mut inner_name_index = mappings_data[mi + 4];
          if inner_source_index >= 0 {
            let inner_source_index = inner_source_index as u32;
            // Check for an identity mapping
            // where we are allowed to adjust the original column
            let inner_chunk = &chunks[idx];
            let inner_generated_column = mappings_data[mi];
            let location_in_chunk = original_column - inner_generated_column;
            if location_in_chunk > 0 {
              let inner_source_content_lines =
                inner_source_content_lines.borrow_mut();
              let original_source_lines =
                match inner_source_content_lines.get(&inner_source_index) {
                  Some(once_cell) => once_cell.get_or_init(|| {
                    let inner_source_contents = inner_source_contents.borrow();
                    match inner_source_contents.get(&inner_source_index) {
                      Some(Some(source_content)) => Some(
                        split_into_lines(source_content)
                          .map(WithIndices::new)
                          .collect(),
                      ),
                      _ => None,
                    }
                  }),
                  None => &None,
                };
              if let Some(original_source_lines) = original_source_lines {
                let original_chunk = original_source_lines
                  .get(inner_original_line as usize - 1)
                  .map(|lines| {
                    let start = inner_original_column as usize;
                    let end = start + location_in_chunk as usize;
                    lines.substring(start, end)
                  });
                if let Some(original_chunk) = original_chunk {
                  if original_chunk.len() <= inner_chunk.len()
                    && inner_chunk.get(..original_chunk.len())
                      == Some(original_chunk)
                  {
                    inner_original_column += location_in_chunk;
                    inner_name_index = -1;
                  }
                }
              }
            }

            // We have a inner mapping to original source

            // emit source when needed and compute global source index
            let mut inner_source_index_mapping =
              inner_source_index_mapping.borrow_mut();
            let mut source_index = inner_source_index_mapping
              .get(&inner_source_index)
              .copied()
              .unwrap_or(-2);
            if source_index == -2 {
              let (source, source_content) = inner_source_index_value_mapping
                .borrow()
                .get(&inner_source_index)
                .cloned()
                .unwrap_or(("".into(), None));
              let mut source_mapping = source_mapping.borrow_mut();
              let mut global_index = source_mapping.get(&source).copied();
              if global_index.is_none() {
                let len = source_mapping.len() as u32;
                source_mapping.insert(source.clone(), len);
                on_source.borrow_mut()(len, source, source_content);
                global_index = Some(len);
              }
              source_index = global_index.unwrap() as i64;
              inner_source_index_mapping
                .insert(inner_source_index, source_index);
            }

            // emit name when needed and compute global name index
            let mut final_name_index = -1;
            if inner_name_index >= 0 {
              let inner_name_index = inner_name_index as u32;
              // when we have a inner name
              let mut inner_name_index_mapping =
                inner_name_index_mapping.borrow_mut();
              final_name_index = inner_name_index_mapping
                .get(&inner_name_index)
                .copied()
                .unwrap_or(-2);
              if final_name_index == -2 {
                if let Some(name) = inner_name_index_value_mapping
                  .borrow()
                  .get(&inner_name_index)
                {
                  let mut global_index = name_mapping.get(name).copied();
                  if global_index.is_none() {
                    let len = name_mapping.len() as u32;
                    name_mapping.insert(name.clone(), len);
                    on_name(len, name.clone());
                    global_index = Some(len);
                  }
                  final_name_index = global_index.unwrap() as i64;
                } else {
                  final_name_index = -1;
                }
                inner_name_index_mapping
                  .insert(inner_name_index, final_name_index);
              }
            } else if name_index >= 0 {
              let name_index = name_index as u32;
              // when we don't have an inner name,
              // but we have an outer name
              // it can be used when inner original code equals to the name
              let inner_source_content_lines =
                inner_source_content_lines.borrow_mut();
              let original_source_lines =
                match inner_source_content_lines.get(&inner_source_index) {
                  Some(once_cell) => once_cell.get_or_init(|| {
                    let inner_source_contents = inner_source_contents.borrow();
                    match inner_source_contents.get(&inner_source_index) {
                      Some(Some(source_content)) => Some(
                        split_into_lines(source_content)
                          .map(WithIndices::new)
                          .collect(),
                      ),
                      _ => None,
                    }
                  }),
                  None => &None,
                };
              if let Some(original_source_lines) = original_source_lines {
                let name_index_value_mapping =
                  name_index_value_mapping.borrow();
                let name =
                  name_index_value_mapping.get(&name_index).cloned().unwrap();
                let original_name = original_source_lines
                  .get(inner_original_line as usize - 1)
                  .map_or("", |i| {
                    let start = inner_original_column as usize;
                    let end = start + name.len();
                    i.substring(start, end)
                  });
                if name == original_name {
                  let mut name_index_mapping = name_index_mapping.borrow_mut();
                  final_name_index =
                    name_index_mapping.get(&name_index).copied().unwrap_or(-2);
                  if final_name_index == -2 {
                    if let Some(name) =
                      name_index_value_mapping.get(&name_index)
                    {
                      let mut global_index = name_mapping.get(name).copied();
                      if global_index.is_none() {
                        let len = name_mapping.len() as u32;
                        name_mapping.insert(name.clone(), len);
                        on_name(len, name.clone());
                        global_index = Some(len);
                      }
                      final_name_index = global_index.unwrap() as i64;
                    } else {
                      final_name_index = -1;
                    }
                    name_index_mapping.insert(name_index, final_name_index);
                  }
                }
              }
            }
            on_chunk(
              chunk,
              Mapping {
                generated_line: mapping.generated_line,
                generated_column: mapping.generated_column,
                original: (source_index >= 0).then_some(OriginalLocation {
                  source_index: source_index as u32,
                  original_line: inner_original_line as u32,
                  original_column: inner_original_column as u32,
                  name_index: (final_name_index >= 0)
                    .then_some(final_name_index as u32),
                }),
              },
            );
            return;
          }
        }

        // We have a mapping to the inner source, but no inner mapping
        if remove_inner_source {
          on_chunk(
            chunk,
            Mapping {
              generated_line: mapping.generated_line,
              generated_column: mapping.generated_column,
              original: None,
            },
          );
          return;
        } else {
          let mut source_index_mapping = source_index_mapping.borrow_mut();
          if source_index_mapping.get(&source_index) == Some(&-2) {
            let mut source_mapping = source_mapping.borrow_mut();
            let mut global_index =
              source_mapping.get(inner_source_name).copied();
            if global_index.is_none() {
              let len = source_mapping.len() as u32;
              source_mapping.insert(source.into(), len);
              on_source.borrow_mut()(
                len,
                Cow::Borrowed(inner_source_name),
                *inner_source.borrow(),
              );
              global_index = Some(len);
            }
            source_index_mapping
              .insert(source_index, global_index.unwrap() as i64);
          }
        }
      }

      let final_source_index = if source_index < 0 {
        -1
      } else {
        let source_index = source_index as u32;
        source_index_mapping
          .borrow()
          .get(&source_index)
          .copied()
          .unwrap_or(-1)
      };
      if final_source_index < 0 {
        // no source, so we make it a generated chunk
        on_chunk(
          chunk,
          Mapping {
            generated_line: mapping.generated_line,
            generated_column: mapping.generated_column,
            original: None,
          },
        );
      } else {
        // Pass through the chunk with mapping
        let mut name_index_mapping = name_index_mapping.borrow_mut();
        let mut final_name_index = if name_index >= 0 {
          let name_index = name_index as u32;
          name_index_mapping.get(&name_index).copied().unwrap_or(-1)
        } else {
          -1
        };
        if final_name_index == -2 {
          let name_index = name_index as u32;
          let name_index_value_mapping = name_index_value_mapping.borrow();
          let name = name_index_value_mapping.get(&name_index).unwrap();
          let mut global_index = name_mapping.get(name).copied();
          if global_index.is_none() {
            let len = name_mapping.len() as u32;
            name_mapping.borrow_mut().insert(name.clone(), len);
            on_name(len, name.clone());
            global_index = Some(len);
          }
          final_name_index = global_index.unwrap() as i64;
          name_index_mapping.insert(name_index, final_name_index);
        }
        on_chunk(
          chunk,
          Mapping {
            generated_line: mapping.generated_line,
            generated_column: mapping.generated_column,
            original: (final_source_index >= 0).then_some(OriginalLocation {
              source_index: final_source_index as u32,
              original_line: original_line as u32,
              original_column: original_column as u32,
              name_index: (final_name_index >= 0)
                .then_some(final_name_index as u32),
            }),
          },
        );
      }
    },
    &mut |i, source, mut source_content| {
      if source == inner_source_name {
        *inner_source_index.borrow_mut() = i as i64;
        let mut inner_source = inner_source.borrow_mut();
        if let Some(inner_source) = inner_source.as_ref() {
          source_content = Some(inner_source);
        } else {
          *inner_source = source_content;
        }
        source_index_mapping.borrow_mut().insert(i, -2);
        stream_chunks_of_source_map(
          source_content.unwrap(),
          inner_source_map,
          &mut |chunk, mapping| {
            let mut inner_source_map_line_data =
              inner_source_map_line_data.borrow_mut();
            let inner_source_map_line_data_len =
              inner_source_map_line_data.len();
            let mapping_generated_line_len = mapping.generated_line as usize;
            if inner_source_map_line_data_len <= mapping_generated_line_len {
              inner_source_map_line_data.reserve(
                mapping_generated_line_len - inner_source_map_line_data_len + 1,
              );
              while inner_source_map_line_data.len()
                <= mapping_generated_line_len
              {
                inner_source_map_line_data.push(SourceMapLineData {
                  mappings_data: Default::default(),
                  chunks: vec![],
                });
              }
            }
            let data = &mut inner_source_map_line_data
              [mapping.generated_line as usize - 1];
            data.mappings_data.reserve(5);
            data.mappings_data.push(mapping.generated_column as i64);
            data.mappings_data.push(
              mapping
                .original
                .as_ref()
                .map_or(-1, |original| original.source_index as i64),
            );
            data.mappings_data.push(
              mapping
                .original
                .as_ref()
                .map_or(-1, |original| original.original_line as i64),
            );
            data.mappings_data.push(
              mapping
                .original
                .as_ref()
                .map_or(-1, |original| original.original_column as i64),
            );
            data.mappings_data.push(
              mapping
                .original
                .and_then(|original| original.name_index)
                .map(Into::into)
                .unwrap_or(-1),
            );
            // SAFETY: final_source is false
            data.chunks.push(chunk.unwrap());
          },
          &mut |i, source, source_content| {
            inner_source_contents.borrow_mut().insert(i, source_content);
            inner_source_content_lines
              .borrow_mut()
              .insert(i, Default::default());
            inner_source_index_mapping.borrow_mut().insert(i, -2);
            inner_source_index_value_mapping
              .borrow_mut()
              .insert(i, (source, source_content));
          },
          &mut |i, name| {
            inner_name_index_mapping.borrow_mut().insert(i, -2);
            inner_name_index_value_mapping.borrow_mut().insert(i, name);
          },
          &MapOptions {
            columns: options.columns,
            final_source: false,
          },
        );
      } else {
        let mut source_mapping = source_mapping.borrow_mut();
        let mut global_index = source_mapping.get(&source).copied();
        if global_index.is_none() {
          let len = source_mapping.len() as u32;
          source_mapping.insert(source.clone(), len);
          on_source.borrow_mut()(len, source, source_content);
          global_index = Some(len);
        }
        source_index_mapping
          .borrow_mut()
          .insert(i, global_index.unwrap() as i64);
      }
    },
    &mut |i, name| {
      name_index_mapping.borrow_mut().insert(i, -2);
      name_index_value_mapping.borrow_mut().insert(i, name);
    },
    options,
  )
}

pub fn stream_and_get_source_and_map<'a, S: StreamChunks<'a>>(
  input_source: &'a S,
  options: &MapOptions,
  on_chunk: OnChunk<'_, 'a>,
  on_source: OnSource<'_, 'a>,
  on_name: OnName<'_, 'a>,
) -> (GeneratedInfo, Option<SourceMap>) {
  let mut mappings_encoder = create_encoder(options.columns);
  let mut sources: Vec<String> = Vec::new();
  let mut sources_content: Vec<String> = Vec::new();
  let mut names: Vec<String> = Vec::new();

  let generated_info = input_source.stream_chunks(
    options,
    &mut |chunk, mapping| {
      mappings_encoder.encode(&mapping);
      on_chunk(chunk, mapping);
    },
    &mut |source_index, source, source_content| {
      let source_index2 = source_index as usize;
      while sources.len() <= source_index2 {
        sources.push("".into());
      }
      sources[source_index2] = source.to_string();
      if let Some(source_content) = source_content {
        while sources_content.len() <= source_index2 {
          sources_content.push("".into());
        }
        sources_content[source_index2] = source_content.to_string();
      }
      on_source(source_index, source, source_content);
    },
    &mut |name_index, name| {
      let name_index2 = name_index as usize;
      while names.len() <= name_index2 {
        names.push("".into());
      }
      names[name_index2] = name.to_string();
      on_name(name_index, name);
    },
  );

  let mappings = mappings_encoder.drain();
  let map = if mappings.is_empty() {
    None
  } else {
    Some(SourceMap::new(mappings, sources, sources_content, names))
  };
  (generated_info, map)
}
