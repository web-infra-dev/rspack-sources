use std::{
  any::{Any, TypeId},
  borrow::Cow,
  fmt::{self, Debug},
  hash::{Hash, Hasher},
  sync::Arc,
};

use crate::{helpers::StreamChunks, rope::Rope, SourceMap};

/// An alias for `Box<dyn Source>`.
pub type BoxSource = Arc<dyn Source>;

/// [Source] abstraction, [webpack-sources docs](https://github.com/webpack/webpack-sources/#source).
pub trait Source:
  StreamChunks + DynHash + AsAny + DynEq + fmt::Debug + Sync + Send
{
  /// Get the source code.
  fn source(&self) -> Cow<str>;

  /// Get the source code as a [Rope].
  fn rope(&self) -> Rope<'_>;

  /// Get the source buffer.
  fn buffer(&self) -> Cow<[u8]>;

  /// Get the size of the source.
  fn size(&self) -> usize;

  /// Get the [SourceMap].
  fn map(&self, options: &MapOptions) -> Option<SourceMap>;

  /// Update hash based on the source.
  fn update_hash(&self, state: &mut dyn Hasher) {
    self.dyn_hash(state);
  }

  /// Writes the source into a writer, preferably a `std::io::BufWriter<std::io::Write>`.
  fn to_writer(&self, writer: &mut dyn std::io::Write) -> std::io::Result<()>;
}

impl Source for BoxSource {
  fn source(&self) -> Cow<str> {
    self.as_ref().source()
  }

  fn rope(&self) -> Rope<'_> {
    self.as_ref().rope()
  }

  fn buffer(&self) -> Cow<[u8]> {
    self.as_ref().buffer()
  }

  fn size(&self) -> usize {
    self.as_ref().size()
  }

  fn map(&self, options: &MapOptions) -> Option<SourceMap> {
    self.as_ref().map(options)
  }

  fn to_writer(&self, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
    self.as_ref().to_writer(writer)
  }
}

impl StreamChunks for BoxSource {
  fn stream_chunks<'a>(
    &'a self,
    options: &MapOptions,
    on_chunk: crate::helpers::OnChunk<'_, 'a>,
    on_source: crate::helpers::OnSource<'_, 'a>,
    on_name: crate::helpers::OnName<'_, 'a>,
  ) -> crate::helpers::GeneratedInfo {
    self
      .as_ref()
      .stream_chunks(options, on_chunk, on_source, on_name)
  }
}

// for `updateHash`
pub trait DynHash {
  fn dyn_hash(&self, state: &mut dyn Hasher);
}

impl<H: Hash> DynHash for H {
  fn dyn_hash(&self, mut state: &mut dyn Hasher) {
    self.hash(&mut state);
  }
}

impl Hash for dyn Source {
  fn hash<H: Hasher>(&self, state: &mut H) {
    self.dyn_hash(state)
  }
}

pub trait AsAny {
  fn as_any(&self) -> &dyn Any;
}

impl<T: Any> AsAny for T {
  fn as_any(&self) -> &dyn Any {
    self
  }
}

pub trait DynEq {
  fn dyn_eq(&self, other: &dyn Any) -> bool;
  fn type_id(&self) -> TypeId;
}

impl<E: Eq + Any> DynEq for E {
  fn dyn_eq(&self, other: &dyn Any) -> bool {
    if let Some(other) = other.downcast_ref::<E>() {
      self == other
    } else {
      false
    }
  }

  fn type_id(&self) -> TypeId {
    TypeId::of::<E>()
  }
}

impl PartialEq for dyn Source {
  fn eq(&self, other: &Self) -> bool {
    if self.as_any().type_id() != other.as_any().type_id() {
      return false;
    }
    self.dyn_eq(other.as_any())
  }
}

impl Eq for dyn Source {}

/// Extension methods for [Source].
pub trait SourceExt {
  /// An alias for [BoxSource::from].
  fn boxed(self) -> BoxSource;
}

impl<T: Source + 'static> SourceExt for T {
  fn boxed(self) -> BoxSource {
    Arc::new(self)
  }
}

/// Options for [Source::map].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MapOptions {
  /// Whether have columns info in generated [SourceMap] mappings.
  pub columns: bool,
  /// Whether the source will have changes, internal used for `ReplaceSource`, etc.
  pub(crate) final_source: bool,
}

impl Default for MapOptions {
  fn default() -> Self {
    Self {
      columns: true,
      final_source: false,
    }
  }
}

impl MapOptions {
  /// Create [MapOptions] with columns.
  pub fn new(columns: bool) -> Self {
    Self {
      columns,
      ..Default::default()
    }
  }
}

/// Represent a [Mapping] information of source map.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Mapping {
  /// Generated line.
  pub generated_line: u32,
  /// Generated column.
  pub generated_column: u32,
  /// Original position information.
  pub original: Option<OriginalLocation>,
}

/// Represent original position information of a [Mapping].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OriginalLocation {
  /// Source index.
  pub source_index: u32,
  /// Original line.
  pub original_line: u32,
  /// Original column.
  pub original_column: u32,
  /// Name index.
  pub name_index: Option<u32>,
}

/// An convenient way to create a [Mapping].
#[macro_export]
macro_rules! m {
  ($gl:expr, $gc:expr, $si:expr, $ol:expr, $oc:expr, $ni:expr) => {{
    let gl: i64 = $gl;
    let gc: i64 = $gc;
    let si: i64 = $si;
    let ol: i64 = $ol;
    let oc: i64 = $oc;
    let ni: i64 = $ni;
    $crate::Mapping {
      generated_line: gl as u32,
      generated_column: gc as u32,
      original: (si >= 0).then(|| $crate::OriginalLocation {
        source_index: si as u32,
        original_line: ol as u32,
        original_column: oc as u32,
        name_index: (ni >= 0).then(|| ni as u32),
      }),
    }
  }};
}

/// An convenient way to create [Mapping]s.
#[macro_export]
macro_rules! mappings {
  ($($mapping:expr),* $(,)?) => {
    ::std::vec![$({
      let mapping = $mapping;
      $crate::m![mapping[0], mapping[1], mapping[2], mapping[3], mapping[4], mapping[5]]
    }),*]
  };
}

#[cfg(test)]
mod tests {
  use std::collections::HashMap;

  use crate::{
    CachedSource, ConcatSource, OriginalSource, RawBufferSource, RawSource,
    RawStringSource, ReplaceSource, SourceMapSource, WithoutOriginalOptions,
  };

  use super::*;

  #[test]
  fn should_not_have_sources_content_field_when_it_is_empty() {
    let map = SourceMap::new(
      ";;",
      vec!["a.js".into()],
      vec!["".into(), "".into(), "".into()],
      vec!["".into(), "".into()],
    )
    .to_json()
    .unwrap();
    assert!(!map.contains("sourcesContent"));
  }

  #[test]
  fn hash_available() {
    let mut state = twox_hash::XxHash64::default();
    RawSource::from("a").hash(&mut state);
    OriginalSource::new("b", "").hash(&mut state);
    SourceMapSource::new(WithoutOriginalOptions {
      value: "c",
      name: "",
      source_map: SourceMap::from_json("{\"mappings\": \";\"}").unwrap(),
    })
    .hash(&mut state);
    ConcatSource::new([RawSource::from("d")]).hash(&mut state);
    CachedSource::new(RawSource::from("e")).hash(&mut state);
    ReplaceSource::new(RawSource::from("f")).hash(&mut state);
    RawSource::from("g").boxed().hash(&mut state);
    RawStringSource::from_static("a").hash(&mut state);
    RawBufferSource::from("a".as_bytes()).hash(&mut state);
    (&RawSource::from("h") as &dyn Source).hash(&mut state);
    ReplaceSource::new(RawSource::from("i").boxed()).hash(&mut state);
    assert_eq!(format!("{:x}", state.finish()), "90b46a65420d1a02");
  }

  #[test]
  fn eq_available() {
    assert_eq!(RawSource::from("a"), RawSource::from("a"));
    assert_eq!(
      RawStringSource::from_static("a"),
      RawStringSource::from_static("a")
    );
    assert_eq!(
      RawBufferSource::from("a".as_bytes()),
      RawBufferSource::from("a".as_bytes())
    );
    assert_eq!(OriginalSource::new("b", ""), OriginalSource::new("b", ""));
    assert_eq!(
      SourceMapSource::new(WithoutOriginalOptions {
        value: "c",
        name: "",
        source_map: SourceMap::from_json("{\"mappings\": \";\"}").unwrap(),
      }),
      SourceMapSource::new(WithoutOriginalOptions {
        value: "c",
        name: "",
        source_map: SourceMap::from_json("{\"mappings\": \";\"}").unwrap(),
      })
    );
    assert_eq!(
      ConcatSource::new([RawSource::from("d")]),
      ConcatSource::new([RawSource::from("d")])
    );
    assert_eq!(
      CachedSource::new(RawSource::from("e")),
      CachedSource::new(RawSource::from("e"))
    );
    assert_eq!(
      ReplaceSource::new(RawSource::from("f")),
      ReplaceSource::new(RawSource::from("f"))
    );
    assert_eq!(&RawSource::from("g").boxed(), &RawSource::from("g").boxed());
    assert_eq!(
      (&RawSource::from("h") as &dyn Source),
      (&RawSource::from("h") as &dyn Source)
    );
    assert_eq!(
      ReplaceSource::new(RawSource::from("i").boxed()),
      ReplaceSource::new(RawSource::from("i").boxed())
    );
    assert_eq!(
      CachedSource::new(RawSource::from("j").boxed()),
      CachedSource::new(RawSource::from("j").boxed())
    );
  }

  #[test]
  #[allow(suspicious_double_ref_op)]
  fn clone_available() {
    let a = RawSource::from("a");
    assert_eq!(a, a.clone());
    let b = OriginalSource::new("b", "");
    assert_eq!(b, b.clone());
    let c = SourceMapSource::new(WithoutOriginalOptions {
      value: "c",
      name: "",
      source_map: SourceMap::from_json("{\"mappings\": \";\"}").unwrap(),
    })
    .boxed();
    assert_eq!(&c, &c.clone());
    let d = ConcatSource::new([RawSource::from("d")]);
    assert_eq!(d, d.clone());
    let e = CachedSource::new(RawSource::from("e")).boxed();
    assert_eq!(&e, &e.clone());
    let f = ReplaceSource::new(RawSource::from("f"));
    assert_eq!(f, f.clone());
    let g = RawSource::from("g").boxed();
    assert_eq!(&g, &g.clone());
    let h = &RawSource::from("h") as &dyn Source;
    assert_eq!(h, h);
    let i = ReplaceSource::new(RawSource::from("i").boxed());
    assert_eq!(i, i.clone());
    let j = CachedSource::new(RawSource::from("j").boxed()).boxed();
    assert_eq!(&j, &j.clone());
    let k = RawStringSource::from_static("k");
    assert_eq!(k, k.clone());
    let l = RawBufferSource::from("l".as_bytes());
    assert_eq!(l, l.clone());
  }

  #[test]
  fn box_dyn_source_use_hashmap_available() {
    let mut map = HashMap::new();
    let a = RawSource::from("a").boxed();
    map.insert(a.clone(), a.clone());
    assert_eq!(map.get(&a).unwrap(), &a);
  }

  #[test]
  #[allow(suspicious_double_ref_op)]
  fn ref_dyn_source_use_hashmap_available() {
    let mut map = HashMap::new();
    let a = &RawSource::from("a") as &dyn Source;
    map.insert(a, a);
    assert_eq!(map.get(&a).unwrap(), &a);
  }

  #[test]
  fn to_writer() {
    let sources =
      ConcatSource::new([RawSource::from("a"), RawSource::from("b")]);
    let mut writer = std::io::BufWriter::new(Vec::new());
    let result = sources.to_writer(&mut writer);
    assert!(result.is_ok());
    assert_eq!(
      String::from_utf8(writer.into_inner().unwrap()).unwrap(),
      "ab"
    );
  }
}
