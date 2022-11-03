//! Rusty [`webpack-sources`](https://github.com/webpack/webpack-sources) port.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod cached_source;
mod concat_source;
mod error;
mod helpers;
mod original_source;
mod raw_source;
mod replace_source;
mod source;
mod source_map_source;
mod vlq;

pub use cached_source::CachedSource;
pub use concat_source::ConcatSource;
pub use error::{Error, Result};
pub use helpers::{
  decode_mappings, encode_mappings, get_map, stream_chunks_of_raw_source,
  stream_chunks_of_source_map, GeneratedInfo, OnChunk, OnName, OnSource,
  StreamChunks,
};
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
