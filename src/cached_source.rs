use std::{
  borrow::Cow,
  hash::{Hash, Hasher},
  sync::{Arc, Mutex, OnceLock},
};

use rustc_hash::FxHasher;

use crate::{
  encoder::create_encoder,
  helpers::{
    stream_chunks_of_raw_source, stream_chunks_of_source_map, GeneratedInfo,
    OnChunk, OnName, OnSource, StreamChunks,
  },
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
pub struct CachedSource {
  inner: Mutex<Option<BoxSource>>,
  cached_buffer: OnceLock<Vec<u8>>,
  cached_source: OnceLock<Arc<str>>,
  cached_hash: OnceLock<u64>,
  cached_full_map: OnceLock<Option<SourceMap>>,
  cached_lines_only_map: OnceLock<Option<SourceMap>>,
}

impl CachedSource {
  /// Create a [CachedSource] with the original [Source].
  pub fn new<T: Source + 'static>(inner: T) -> Self {
    Self {
      inner: Mutex::new(Some(inner.boxed())),
      cached_buffer: Default::default(),
      cached_source: Default::default(),
      cached_hash: Default::default(),
      cached_full_map: Default::default(),
      cached_lines_only_map: Default::default(),
    }
  }

  fn stream_and_get_source_and_map<'a>(
    &'a self,
    input_source: &BoxSource,
    options: &MapOptions,
    on_chunk: OnChunk<'_, 'a>,
    on_source: OnSource<'_, 'a>,
    on_name: OnName<'_, 'a>,
  ) -> GeneratedInfo {
    let code = self
      .cached_source
      .get_or_init(|| input_source.source().into());
    let mut code_start = 0;
    let mut code_end = 0;

    self.cached_buffer.get_or_init(|| code.as_bytes().to_vec());

    let mut mappings_encoder = create_encoder(options.columns);
    let mut sources: Vec<String> = Vec::new();
    let mut sources_content: Vec<String> = Vec::new();
    let mut names: Vec<String> = Vec::new();

    let generated_info = input_source.stream_chunks(
      options,
      &mut |chunk, mapping| {
        mappings_encoder.encode(&mapping);
        if let Some(chunk) = chunk {
          code_start += chunk.len();
          code_end += chunk.len();
          on_chunk(Some(Cow::Borrowed(&code[code_start..code_end])), mapping);
        } else {
          on_chunk(Some(Cow::Borrowed("")), mapping);
        }
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
        #[allow(unsafe_code)]
        // SAFETY: the `sources` will be stored in either `self.cached_full_map` or `self.cached_lines_only_map`.
        // As long as the instance containing `self` remains valid within the lifetime `'a`, the `sources` data stored therein will also be valid.
        let source = unsafe {
          std::mem::transmute::<&String, &'a String>(&sources[source_index2])
        };
        #[allow(unsafe_code)]
        // SAFETY: the `sources_content` will be stored in either `self.cached_full_map` or `self.cached_lines_only_map`.
        // As long as the instance containing `self` remains valid within the lifetime `'a`, the `sources_content` data stored therein will also be valid.
        let source_content = unsafe {
          std::mem::transmute::<&String, &'a String>(
            &sources_content[source_index2],
          )
        };
        on_source(source_index, Cow::Borrowed(source), Some(source_content));
      },
      &mut |name_index, name| {
        let name_index2 = name_index as usize;
        while names.len() <= name_index2 {
          names.push("".into());
        }
        names[name_index2] = name.to_string();
        #[allow(unsafe_code)]
        // SAFETY: the `names` will be stored in either `self.cached_full_map` or `self.cached_lines_only_map`.
        // As long as the instance containing `self` remains valid within the lifetime `'a`, the `names` data stored therein will also be valid.
        let name = unsafe {
          std::mem::transmute::<&String, &'a String>(&names[name_index2])
        };
        on_name(name_index, Cow::Borrowed(name));
      },
    );

    let mappings = mappings_encoder.drain();
    let map = if mappings.is_empty() {
      None
    } else {
      Some(SourceMap::new(mappings, sources, sources_content, names))
    };

    if options.columns {
      self.cached_full_map.get_or_init(|| map);
    } else {
      self.cached_lines_only_map.get_or_init(|| map);
    }

    generated_info
  }
}

impl Source for CachedSource {
  fn source(&self) -> Cow<str> {
    let cached = self.cached_source.get_or_init(|| {
      let original = self.inner.lock().unwrap();
      original.as_ref().unwrap().source().into()
    });
    Cow::Borrowed(cached)
  }

  fn buffer(&self) -> Cow<[u8]> {
    let mut original = self.inner.lock().unwrap();

    let cached = self
      .cached_buffer
      .get_or_init(|| original.as_ref().unwrap().buffer().into());

    if self.cached_hash.get().is_some()
      && self.cached_full_map.get().is_some()
      && self.cached_source.get().is_some()
    {
      original.take();
    }

    Cow::Borrowed(cached)
  }

  fn size(&self) -> usize {
    self.source().len()
  }

  fn map(&self, options: &MapOptions) -> Option<SourceMap> {
    let mut original = self.inner.lock().unwrap();

    if options.columns {
      self
        .cached_full_map
        .get_or_init(|| {
          let map = original.as_ref().unwrap().map(options);

          if self.cached_buffer.get().is_some()
            && self.cached_source.get().is_some()
            && self.cached_hash.get().is_some()
          {
            original.take();
          }

          map
        })
        .clone()
    } else {
      self
        .cached_lines_only_map
        .get_or_init(|| {
          if let Some(map) = self.cached_full_map.get() {
            if let Some(map) = map {
              let mut lines_only_map = map.clone();
              let mut mappings_encoder = create_encoder(options.columns);
              map
                .decoded_mappings()
                .for_each(|mapping| mappings_encoder.encode(&mapping));
              let mappings = mappings_encoder.drain();
              lines_only_map.set_mappings(mappings);
              if lines_only_map.mappings().is_empty() {
                None
              } else {
                Some(lines_only_map)
              }
            } else {
              None
            }
          } else {
            original.as_ref().unwrap().map(options)
          }
        })
        .clone()
    }
  }
}

impl StreamChunks<'_> for CachedSource {
  fn stream_chunks<'a>(
    &'a self,
    options: &MapOptions,
    on_chunk: crate::helpers::OnChunk<'_, 'a>,
    on_source: crate::helpers::OnSource<'_, 'a>,
    on_name: crate::helpers::OnName<'_, 'a>,
  ) -> crate::helpers::GeneratedInfo {
    let cache = if options.columns {
      &self.cached_full_map
    } else {
      &self.cached_lines_only_map
    };
    if let Some(map) = cache.get() {
      let source = self.cached_source.get_or_init(|| {
        let original = self.inner.lock().unwrap();
        original.as_ref().unwrap().source().into()
      });
      if let Some(map) = map.as_ref() {
        #[allow(unsafe_code)]
        // SAFETY: We guarantee that once a `SourceMap` is stored in the cache, it will never be removed.
        // Therefore, even if we force its lifetime to be longer, the reference remains valid.
        // This is based on the following assumptions:
        // 1. `SourceMap` will be valid for the entire duration of the application.
        // 2. The cached `SourceMap` will not be manually removed or replaced, ensuring the reference's safety.
        let map =
          unsafe { std::mem::transmute::<&SourceMap, &'a SourceMap>(map) };
        stream_chunks_of_source_map(
          source, map, on_chunk, on_source, on_name, options,
        )
      } else {
        stream_chunks_of_raw_source(
          source, options, on_chunk, on_source, on_name,
        )
      }
    } else {
      let mut original = self.inner.lock().unwrap();

      let generated_info = self.stream_and_get_source_and_map(
        original.as_ref().unwrap(),
        options,
        on_chunk,
        on_source,
        on_name,
      );

      if self.cached_buffer.get().is_some()
        && options.columns
        && self.cached_source.get().is_some()
        && self.cached_hash.get().is_some()
      {
        original.take();
      }

      generated_info
    }
  }
}

impl Hash for CachedSource {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    let mut original = self.inner.lock().unwrap();

    (self.cached_hash.get_or_init(|| {
      let mut hasher = FxHasher::default();
      original.as_ref().unwrap().hash(&mut hasher);
      hasher.finish()
    }))
    .hash(state);

    if self.cached_buffer.get().is_some()
      && self.cached_full_map.get().is_some()
      && self.cached_source.get().is_some()
    {
      original.take();
    }
  }
}

impl PartialEq for CachedSource {
  fn eq(&self, other: &Self) -> bool {
    if std::ptr::eq(self, other) {
      return true;
    }
    self.cached_buffer.get() == other.cached_buffer.get()
      && self.cached_full_map.get() == other.cached_full_map.get()
      && self.cached_lines_only_map.get() == other.cached_lines_only_map.get()
      && self.cached_source.get() == other.cached_source.get()
      && self.cached_hash.get() == other.cached_hash.get()
      && *self.inner.lock().unwrap() == *other.inner.lock().unwrap()
  }
}

impl Eq for CachedSource {}

impl std::fmt::Debug for CachedSource {
  fn fmt(
    &self,
    f: &mut std::fmt::Formatter<'_>,
  ) -> Result<(), std::fmt::Error> {
    f.debug_struct("CachedSource")
      .field("inner", &self.inner)
      .field("cached_buffer", &self.cached_buffer.get().is_some())
      .field("cached_source", &self.cached_source.get().is_some())
      .field("cached_full_map", &(self.cached_full_map.get().is_some()))
      .field(
        "cached_lines_only_map",
        &(self.cached_lines_only_map.get().is_some()),
      )
      .finish()
  }
}

#[cfg(test)]
mod tests {
  use crate::{
    ConcatSource, OriginalSource, RawSource, SourceExt, SourceMapSource,
    WithoutOriginalOptions,
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
}
