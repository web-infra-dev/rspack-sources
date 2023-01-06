//! Rusty [`webpack-sources`](https://github.com/webpack/webpack-sources) port.

#![forbid(unsafe_code)]
// #![deny(missing_docs)]

mod cached_source;
mod concat_source;
mod error;
// mod helpers;
pub mod helpers;
mod original_source;
mod raw_source;
mod replace_source;
mod source;
mod source_map_source;
mod vlq;


pub use cached_source::CachedSource;
pub use concat_source::ConcatSource;
pub use error::{Error, Result};
pub use original_source::OriginalSource;
pub use raw_source::RawSource;
pub use replace_source::ReplaceSource;
pub use source::{
  BoxSource, MapOptions, Mapping, OriginalLocation, Source, SourceExt,
  SourceMap,
};
pub use source_map_source::{
  SourceMapSource, SourceMapSourceOptions, WithoutOriginalOptions,
};

/// Reexport [StreamChunks] related types.
pub mod stream_chunks {
  pub use super::helpers::{
    stream_chunks_default, GeneratedInfo, OnChunk, OnName, OnSource,
    StreamChunks,
  };
}

pub fn substring1(str: &str, _start: usize, _end: usize) -> &str {
  let chars = str.chars();
  let start_index = chars.clone().fold(0, |acc, cur| acc + cur.len_utf8());


  dbg!(&chars.clone().take(1).next());
  let end_index = chars
    .take(1)
    .fold(start_index, |acc, cur| acc + cur.len_utf8());
  dbg!(start_index, end_index);
  &str[start_index..end_index]
}
