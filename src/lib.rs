//! Rusty [`webpack-sources`](https://github.com/webpack/webpack-sources) port.

mod cached_source;
mod concat_source;
mod decoder;
mod encoder;
mod error;
mod helpers;
mod linear_map;
mod object_pool;
mod original_source;
mod raw_source;
mod replace_source;
mod rope;
mod source;
mod source_content_lines;
mod source_map_source;
mod with_indices;

pub use cached_source::CachedSource;
pub use concat_source::ConcatSource;
pub use error::{Error, Result};
pub use original_source::OriginalSource;
pub use raw_source::{RawBufferSource, RawStringSource};
pub use replace_source::{ReplaceSource, ReplacementEnforce};
pub use rope::Rope;
pub use source::{
  BoxSource, MapOptions, Mapping, OriginalLocation, Source, SourceExt,
  SourceMap, SourceValue,
};
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

pub use object_pool::with_object_pool_scope;
