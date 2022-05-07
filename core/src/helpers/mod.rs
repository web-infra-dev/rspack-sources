use sourcemap::SourceMap;

pub mod mapping;

use crate::source::GenMapOption;

pub fn get_map(option: GenMapOption) -> Option<SourceMap> {
  let tokens = vec![];
  let sources = vec![];
  let sources_content = vec![];
  let names = vec![];

  match tokens.len() {
    0 => None,
    _ => Some(SourceMap::new(
      Some("x".to_string()),
      tokens,
      names,
      sources,
      match sources_content.len() {
        0 => None,
        _ => Some(sources_content),
      },
    )),
  }
}

pub fn stream_chunks_of_combined_source_map<C, S, N>(
  source: String,
  source_map: SourceMap,
  inner_source_name: String,
  inner_source: String,
  inner_source_map: SourceMap,
  remove_inner_source: bool,
  on_chunk: C,
  on_source: S,
  on_name: N,
  final_source: bool,
  columns: bool,
) where
  C: Fn(i32, i32, i32, i32, i32, i32),
  S: Fn(i32, String, String),
  N: Fn(i32, String),
{
}

pub fn stream_chunks_of_source_map<C, S, N>(
  source: String,
  source_map: SourceMap,
  on_chunk: C,
  on_source: S,
  on_name: N,
  final_source: bool,
  columns: bool,
) where
  C: Fn(i32, i32, i32, i32, i32, i32),
  S: Fn(i32, String, String),
  N: Fn(i32, String),
{
  if columns {
    if final_source {
      return stream_chunks_of_source_map_final(source, source_map, on_chunk, on_source, on_name);
    } else {
      return stream_chunks_of_source_map_full(source, source_map, on_chunk, on_source, on_name);
    }
  }
}

pub fn stream_chunks_of_source_map_final<C, S, N>(
  source: String,
  source_map: SourceMap,
  on_chunk: C,
  on_source: S,
  on_name: N,
) where
  C: Fn(i32, i32, i32, i32, i32, i32),
  S: Fn(i32, String, String),
  N: Fn(i32, String),
{
  let mut lines = source.lines();
}

pub fn stream_chunks_of_source_map_full<C, S, N>(
  source: String,
  source_map: SourceMap,
  on_chunk: C,
  on_source: S,
  on_name: N,
) where
  C: Fn(i32, i32, i32, i32, i32, i32),
  S: Fn(i32, String, String),
  N: Fn(i32, String),
{
  let mut lines = source.lines();
  if lines.size_hint().0 == 0 {
    // return ChunkOfSourceMap {
    //   generated_line: 1,
    //   generated_column: 0,
    // };
  }
}

pub struct ChunkOfSourceMap {
  generated_line: i32,
  generated_column: i32,
}
