//! Rusty [`webpack-sources`](https://github.com/webpack/webpack-sources) port.

mod cached_source;
mod concat_source;
mod decoder;
mod encoder;
mod error;
mod helpers;
mod linear_map;
mod original_source;
mod raw_source;
mod replace_source;
mod rope;
mod source;
mod source_map;
mod source_map_source;
mod with_indices;

pub use cached_source::CachedSource;
pub use concat_source::ConcatSource;
pub use error::{Error, Result};
pub use original_source::OriginalSource;
pub use raw_source::{RawBufferSource, RawSource, RawStringSource};
pub use replace_source::{ReplaceSource, ReplacementEnforce};
pub use rope::Rope;
pub use source::{
  BoxSource, MapOptions, Mapping, OriginalLocation, Source, SourceExt,
};
pub use source_map::SourceMap;
pub use source_map_source::{
  SourceMapSource, SourceMapSourceOptions, WithoutOriginalOptions,
};

/// Reexport `StreamChunks` related types.
pub mod stream_chunks {
  pub use super::helpers::{
    stream_chunks_default, GeneratedInfo, OnChunk, OnName, OnSource,
    StreamChunks,
  };
}

pub use helpers::{decode_mappings, encode_mappings};
