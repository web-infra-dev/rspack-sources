#![deny(clippy::all)]

mod result;

pub mod cached_source;
pub mod concat_source;
pub mod helpers;
pub mod source;
pub mod source_map_source;

pub use concat_source::{ConcatSource, ConcattableSource};
pub use source::Source;
pub use source_map_source::{SourceMapSource, SourceMapSourceOptions, SourceMapSourceSliceOptions};
