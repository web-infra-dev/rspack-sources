use std::{
  borrow::Cow,
  hash::{Hash, Hasher},
  sync::OnceLock,
};

use rustc_hash::FxHasher;

use crate::{
  helpers::{
    stream_and_get_source_and_map, stream_chunks_of_raw_source,
    stream_chunks_of_source_map, StreamChunks,
  },
  rope::Rope,
  BoxSource, MapOptions, Source, SourceExt, SourceMap,
};

/// It tries to reused cached results from other methods to avoid calculations,
/// usually used after modify is finished.
///
/// - [webpack-sources docs](https://github.com/webpack/webpack-sources/#cachedsource).
///
/// ```
/// use rspack_sources::{
///   BoxSource, CachedSource, ConcatSource, MapOptions, OriginalSource,
///   RawSource, Source, SourceExt, SourceMap,
/// };
///
/// let mut concat = ConcatSource::new([
///   RawSource::from("Hello World\n".to_string()).boxed(),
///   OriginalSource::new(
///     "console.log('test');\nconsole.log('test2');\n",
///     "console.js",
///   )
///   .boxed(),
/// ]);
/// concat.add(OriginalSource::new("Hello2\n", "hello.md"));
///
/// let cached = CachedSource::new(concat);
///
/// assert_eq!(
///   cached.source(),
///   "Hello World\nconsole.log('test');\nconsole.log('test2');\nHello2\n"
/// );
/// // second time will be fast.
/// assert_eq!(
///   cached.source(),
///   "Hello World\nconsole.log('test');\nconsole.log('test2');\nHello2\n"
/// );
/// ```

#[derive(Debug)]
struct CachedSourceOwner {
  inner: BoxSource,
  cached_hash: OnceLock<u64>,
}

#[derive(Debug)]
struct CachedSourceDependent<'a> {
  cached_colomns_map: OnceLock<Option<SourceMap<'static>>>,
  cached_line_only_map: OnceLock<Option<SourceMap<'static>>>,
  phantom: std::marker::PhantomData<&'a ()>,
}

self_cell::self_cell!(
  struct CachedSourceCell {
    owner: CachedSourceOwner,

    #[covariant]
    dependent: CachedSourceDependent,
  }

  impl { Debug }
);

/// A wrapper around any [`Source`] that caches expensive computations to improve performance.
pub struct CachedSource(CachedSourceCell);

impl CachedSource {
  /// Create a [CachedSource] with the original [Source].
  pub fn new<T: SourceExt>(inner: T) -> Self {
    let owner = CachedSourceOwner {
      inner: inner.boxed(),
      cached_hash: OnceLock::new(),
    };
    Self(CachedSourceCell::new(owner, |_| CachedSourceDependent {
      cached_colomns_map: Default::default(),
      cached_line_only_map: Default::default(),
      phantom: std::marker::PhantomData,
    }))
  }

  /// Get the original [Source].
  pub fn original(&self) -> &BoxSource {
    &self.0.borrow_owner().inner
  }
}

impl Source for CachedSource {
  fn source(&self) -> Cow<str> {
    self.0.borrow_owner().inner.source()
  }

  fn rope(&self) -> Rope<'_> {
    self.0.borrow_owner().inner.rope()
  }

  fn buffer(&self) -> Cow<[u8]> {
    let mut buffer = vec![];
    self.to_writer(&mut buffer).unwrap();
    Cow::Owned(buffer)
  }

  fn size(&self) -> usize {
    self.0.borrow_owner().inner.size()
  }

  fn map<'a>(&'a self, options: &MapOptions) -> Option<Cow<'a, SourceMap<'a>>> {
    if options.columns {
      self.0.with_dependent(|owner, dependent| {
        dependent
          .cached_colomns_map
          .get_or_init(|| {
            owner
              .inner
              .map(options)
              .map(|m| m.as_ref().clone().into_owned())
          })
          .as_ref()
          .map(Cow::Borrowed)
      })
    } else {
      self.0.with_dependent(|owner, dependent| {
        dependent
          .cached_line_only_map
          .get_or_init(|| {
            owner
              .inner
              .map(options)
              .map(|m| m.as_ref().clone().into_owned())
          })
          .as_ref()
          .map(Cow::Borrowed)
      })
    }
  }

  fn to_writer(&self, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
    self.0.borrow_owner().inner.to_writer(writer)
  }
}

impl StreamChunks for CachedSource {
  fn stream_chunks<'a>(
    &'a self,
    options: &MapOptions,
    on_chunk: crate::helpers::OnChunk<'_, 'a>,
    on_source: crate::helpers::OnSource<'_, 'a>,
    on_name: crate::helpers::OnName<'_, 'a>,
  ) -> crate::helpers::GeneratedInfo {
    let cached = if options.columns {
      self.0.borrow_dependent().cached_colomns_map.get()
    } else {
      self.0.borrow_dependent().cached_line_only_map.get()
    };
    match cached {
      Some(Some(map)) => {
        let source = self.0.borrow_owner().inner.rope();
        stream_chunks_of_source_map(
          source, map, on_chunk, on_source, on_name, options,
        )
      }
      Some(None) => {
        let source = self.0.borrow_owner().inner.rope();
        stream_chunks_of_raw_source(
          source, options, on_chunk, on_source, on_name,
        )
      }
      None => {
        if options.columns {
          self.0.with_dependent(|owner, dependent| {
            let (generated_info, map) = stream_and_get_source_and_map(
          &owner.inner,
          options,
          on_chunk,
          on_source,
          on_name,
        );
            dependent.cached_colomns_map.get_or_init(|| {
              unsafe { std::mem::transmute::<Option<SourceMap>, Option<SourceMap<'static>>>(map) }
            });
            generated_info
          })
        } else {
          self.0.with_dependent(|owner, dependent| {
            let (generated_info, map) = stream_and_get_source_and_map(
          &owner.inner,
          options,
          on_chunk,
          on_source,
          on_name,
        );
        dependent.cached_line_only_map.get_or_init(|| {
              unsafe { std::mem::transmute::<Option<SourceMap>, Option<SourceMap<'static>>>(map) }
            });
        generated_info
          })
        }
      }
    }
  }
}

impl Hash for CachedSource {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    let owner = self.0.borrow_owner();
    (owner.cached_hash.get_or_init(|| {
      let mut hasher = FxHasher::default();
      owner.inner.hash(&mut hasher);
      hasher.finish()
    }))
    .hash(state);
  }
}

impl PartialEq for CachedSource {
  fn eq(&self, other: &Self) -> bool {
    &self.0.borrow_owner().inner == &other.0.borrow_owner().inner
  }
}

impl Eq for CachedSource {}

impl std::fmt::Debug for CachedSource {
  fn fmt(
    &self,
    f: &mut std::fmt::Formatter<'_>,
  ) -> Result<(), std::fmt::Error> {
    let indent = f.width().unwrap_or(0);
    let indent_str = format!("{:indent$}", "", indent = indent);

    writeln!(f, "{indent_str}CachedSource::new(")?;
    writeln!(
      f,
      "{indent_str}{:indent$?}",
      self.0.borrow_owner().inner,
      indent = indent + 2
    )?;
    write!(f, "{indent_str}).boxed()")
  }
}

#[cfg(test)]
mod tests {
  use crate::{
    ConcatSource, OriginalSource, RawBufferSource, RawSource, ReplaceSource,
    SourceExt, SourceMapSource, WithoutOriginalOptions,
  };

  use super::*;

  #[test]
  fn line_number_should_not_add_one() {
    let source = ConcatSource::new([
      CachedSource::new(RawSource::from("\n")).boxed(),
      SourceMapSource::new(WithoutOriginalOptions {
        value: "\nconsole.log(1);\n".to_string(),
        name: "index.js".to_string(),
        source_map: SourceMap::new(
          ";AACA",
          vec!["index.js".into()],
          vec!["// DELETE IT\nconsole.log(1)".into()],
          vec![],
        ),
      })
      .boxed(),
    ]);
    let map = source.map(&Default::default()).unwrap();
    assert_eq!(map.mappings(), ";;AACA");
  }

  #[test]
  fn should_return_the_correct_size_for_binary_files() {
    let source = OriginalSource::new(
      String::from_utf8(vec![0; 256]).unwrap(),
      "file.wasm",
    );
    let cached_source = CachedSource::new(source);

    assert_eq!(cached_source.size(), 256);
    assert_eq!(cached_source.size(), 256);
  }

  #[test]
  fn should_return_the_correct_size_for_cached_binary_files() {
    let source = OriginalSource::new(
      String::from_utf8(vec![0; 256]).unwrap(),
      "file.wasm",
    );
    let cached_source = CachedSource::new(source);

    cached_source.source();
    assert_eq!(cached_source.size(), 256);
    assert_eq!(cached_source.size(), 256);
  }

  #[test]
  fn should_return_the_correct_size_for_text_files() {
    let source = OriginalSource::new("TestTestTest", "file.js");
    let cached_source = CachedSource::new(source);

    assert_eq!(cached_source.size(), 12);
    assert_eq!(cached_source.size(), 12);
  }

  #[test]
  fn should_return_the_correct_size_for_cached_text_files() {
    let source = OriginalSource::new("TestTestTest", "file.js");
    let cached_source = CachedSource::new(source);

    cached_source.source();
    assert_eq!(cached_source.size(), 12);
    assert_eq!(cached_source.size(), 12);
  }

  #[test]
  fn should_produce_correct_output_for_cached_raw_source() {
    let map_options = MapOptions {
      columns: true,
      final_source: true,
    };

    let source = RawSource::from("Test\nTest\nTest\n");
    let mut on_chunk_count = 0;
    let mut on_source_count = 0;
    let mut on_name_count = 0;
    let generated_info = source.stream_chunks(
      &map_options,
      &mut |_chunk, _mapping| {
        on_chunk_count += 1;
      },
      &mut |_source_index, _source, _source_content| {
        on_source_count += 1;
      },
      &mut |_name_index, _name| {
        on_name_count += 1;
      },
    );

    let cached_source = CachedSource::new(source);
    cached_source.stream_chunks(
      &map_options,
      &mut |_chunk, _mapping| {},
      &mut |_source_index, _source, _source_content| {},
      &mut |_name_index, _name| {},
    );

    let mut cached_on_chunk_count = 0;
    let mut cached_on_source_count = 0;
    let mut cached_on_name_count = 0;
    let cached_generated_info = cached_source.stream_chunks(
      &map_options,
      &mut |_chunk, _mapping| {
        cached_on_chunk_count += 1;
      },
      &mut |_source_index, _source, _source_content| {
        cached_on_source_count += 1;
      },
      &mut |_name_index, _name| {
        cached_on_name_count += 1;
      },
    );

    assert_eq!(on_chunk_count, cached_on_chunk_count);
    assert_eq!(on_source_count, cached_on_source_count);
    assert_eq!(on_name_count, cached_on_name_count);
    assert_eq!(generated_info, cached_generated_info);
  }

  #[test]
  fn should_have_correct_buffer_if_cache_buffer_from_cache_source() {
    let buf = vec![128u8];
    let source = CachedSource::new(RawSource::from(buf.clone()));

    source.source();
    assert_eq!(source.buffer(), buf.as_slice());
  }

  #[test]
  fn hash_should_different_when_map_are_different() {
    let hash1 = {
      let mut source =
        ReplaceSource::new(OriginalSource::new("Hello", "hello.txt").boxed());
      source.insert(5, " world", None);
      let cache = CachedSource::new(source);
      let mut hasher = FxHasher::default();
      cache.hash(&mut hasher);
      hasher.finish()
    };

    let hash2 = {
      let source = OriginalSource::new("Hello world", "hello.txt").boxed();
      let cache = CachedSource::new(source);
      let mut hasher = FxHasher::default();
      cache.hash(&mut hasher);
      hasher.finish()
    };

    assert!(hash1 != hash2);
  }

  #[test]
  fn size_over_a_raw_buffer_source() {
    // buffer from PNG
    let raw =
      RawBufferSource::from(vec![137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13]);
    let raw_size = raw.size();
    let cached = CachedSource::new(raw.boxed());
    let cached_size = cached.size();
    assert_eq!(raw_size, cached_size);
  }
}
