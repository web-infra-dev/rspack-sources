//! Rusty [`webpack-sources`](https://github.com/webpack/webpack-sources) port.

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
mod with_indices;
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

/// Reexport `StreamChunks` related types.
pub mod stream_chunks {
  pub use super::helpers::{
    stream_chunks_default, GeneratedInfo, OnChunk, OnName, OnSource,
    StreamChunks,
  };
}
