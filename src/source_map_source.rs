use std::{
  borrow::Cow,
  hash::{Hash, Hasher},
  sync::Arc,
};

use crate::{
  helpers::{
    get_map, stream_chunks_of_combined_source_map, stream_chunks_of_source_map,
    StreamChunks,
  },
  DecodableSourceMap, MapOptions, Source, SourceMap,
};

/// Options for [SourceMapSource::new].
pub struct SourceMapSourceOptions<V, N> {
  /// The source code.
  pub value: V,
  /// Name of the file.
  pub name: N,
  /// The source map of the source code.
  pub source_map: Box<dyn DecodableSourceMap>,
  /// The original source code.
  pub original_source: Option<String>,
  /// The original source map.
  pub inner_source_map: Option<Box<dyn DecodableSourceMap>>,
  /// Whether remove the original source.
  pub remove_original_source: bool,
}

/// An convenient options for [SourceMapSourceOptions], `original_source` and
/// `inner_source_map` will be `None`, `remove_original_source` will be false.
pub struct WithoutOriginalOptions<V, N> {
  /// The source code.
  pub value: V,
  /// Name of the file.
  pub name: N,
  /// The source map of the source code.
  pub source_map: Box<dyn DecodableSourceMap>,
}

impl<V, N> From<WithoutOriginalOptions<V, N>> for SourceMapSourceOptions<V, N> {
  fn from(options: WithoutOriginalOptions<V, N>) -> Self {
    Self {
      value: options.value,
      name: options.name,
      source_map: options.source_map,
      original_source: None,
      inner_source_map: None,
      remove_original_source: false,
    }
  }
}

/// Represents source code with source map, optionally having an additional
/// source map for the original source.
///
/// - [webpack-sources docs](https://github.com/webpack/webpack-sources/#sourcemapsource).
pub struct SourceMapSource {
  value: String,
  name: String,
  source_map: Box<dyn DecodableSourceMap>,
  original_source: Option<String>,
  inner_source_map: Option<Box<dyn DecodableSourceMap>>,
  remove_original_source: bool,
}

impl SourceMapSource {
  /// Create a [SourceMapSource] with [SourceMapSourceOptions].
  pub fn new<V, N, O>(options: O) -> Self
  where
    V: Into<String>,
    N: Into<String>,
    O: Into<SourceMapSourceOptions<V, N>>,
  {
    let options = options.into();
    Self {
      value: options.value.into(),
      name: options.name.into(),
      source_map: options.source_map,
      original_source: options.original_source,
      inner_source_map: options.inner_source_map,
      remove_original_source: options.remove_original_source,
    }
  }
}

impl Source for SourceMapSource {
  fn source(&self) -> Cow<str> {
    Cow::Borrowed(&self.value)
  }

  fn buffer(&self) -> Cow<[u8]> {
    Cow::Borrowed(self.value.as_bytes())
  }

  fn size(&self) -> usize {
    self.value.len()
  }

  fn map(&self, options: &MapOptions) -> Option<SourceMap> {
    if self.inner_source_map.is_none() {
      return Some(SourceMap::new(
        self
          .source_map
          .file()
          .map(|file| Arc::from(file.to_string())),
        Arc::from(self.source_map.mappings().to_string()),
        self
          .source_map
          .sources()
          .map(|source| Arc::from(source.to_string()))
          .collect::<Vec<_>>(),
        self
          .source_map
          .sources_content()
          .map(|content| Arc::from(content.to_string()))
          .collect::<Vec<_>>(),
        self
          .source_map
          .names()
          .map(|name| Arc::from(name.to_string()))
          .collect::<Vec<_>>(),
      ));
    }
    get_map(self, options)
  }

  fn to_writer(&self, writer: &mut dyn std::io::Write) -> std::io::Result<()> {
    writer.write_all(self.value.as_bytes())
  }
}

impl Hash for SourceMapSource {
  fn hash<H: Hasher>(&self, state: &mut H) {
    "SourceMapSource".hash(state);
    self.buffer().hash(state);
    self.source_map.hash(state);
    self.original_source.hash(state);
    self.inner_source_map.hash(state);
    self.remove_original_source.hash(state);
  }
}

impl PartialEq for SourceMapSource {
  fn eq(&self, other: &Self) -> bool {
    self.value == other.value
      && self.name == other.name
      && &self.source_map == &other.source_map
      && self.original_source == other.original_source
      && self.inner_source_map == other.inner_source_map
      && self.remove_original_source == other.remove_original_source
  }
}

impl Eq for SourceMapSource {}

impl Clone for SourceMapSource {
  fn clone(&self) -> Self {
    Self {
      value: self.value.clone(),
      name: self.name.clone(),
      source_map: self.source_map.clone(),
      original_source: self.original_source.clone(),
      inner_source_map: self.inner_source_map.clone(),
      remove_original_source: self.remove_original_source.clone(),
    }
  }
}

impl std::fmt::Debug for SourceMapSource {
  fn fmt(
    &self,
    f: &mut std::fmt::Formatter<'_>,
  ) -> Result<(), std::fmt::Error> {
    f.debug_struct("SourceMapSource")
      .field("name", &self.name)
      .field("value", &self.value.chars().take(50).collect::<String>())
      .finish()
  }
}

impl<'a> StreamChunks<'a> for SourceMapSource {
  fn stream_chunks(
    &'a self,
    options: &MapOptions,
    on_chunk: crate::helpers::OnChunk<'_, 'a>,
    on_source: crate::helpers::OnSource<'_, 'a>,
    on_name: crate::helpers::OnName<'_, 'a>,
  ) -> crate::helpers::GeneratedInfo {
    if let Some(inner_source_map) = &self.inner_source_map {
      stream_chunks_of_combined_source_map(
        &self.value,
        self.source_map.as_ref(),
        &self.name,
        self.original_source.as_deref(),
        inner_source_map.as_ref(),
        self.remove_original_source,
        on_chunk,
        on_source,
        on_name,
        options,
      )
    } else {
      stream_chunks_of_source_map(
        &self.value,
        self.source_map.as_ref(),
        on_chunk,
        on_source,
        on_name,
        options,
      )
    }
  }
}

#[cfg(test)]
mod tests {
  use std::sync::Arc;

  use crate::{
    source::DecodableSourceMapExt, CachedSource, ConcatSource, OriginalSource,
    RawSource, ReplaceSource, SourceExt,
  };

  use super::*;

  #[test]
  fn map_correctly() {
    let inner_source_code = "Hello World\nis a test string\n";
    let inner_source = ConcatSource::new([
      OriginalSource::new(inner_source_code, "hello-world.txt").boxed(),
      OriginalSource::new("Translate: ", "header.txt").boxed(),
      RawSource::from("Other text").boxed(),
    ]);
    let source_r_code =
      "Translated: Hallo Welt\nist ein test Text\nAnderer Text";
    let source_r_map = SourceMap::from_json(
      r#"{
        "version": 3,
        "sources": [ "text" ],
        "names": [ "Hello", "World", "nope" ],
        "mappings": "YAAAA,K,CAAMC;AACNC,O,MAAU;AACC,O,CAAM",
        "file": "translated.txt",
        "sourcesContent": [ "Hello World\nis a test string\n" ]
      }"#,
    )
    .unwrap();
    let sms1 = SourceMapSource::new(SourceMapSourceOptions {
      value: source_r_code,
      name: "text",
      source_map: Box::new(source_r_map.clone()),
      original_source: Some(inner_source.source().to_string()),
      inner_source_map: inner_source
        .map(&MapOptions::default())
        .map(|source_map| Box::new(source_map) as Box<dyn DecodableSourceMap>),
      remove_original_source: false,
    });
    let sms2 = SourceMapSource::new(SourceMapSourceOptions {
      value: source_r_code,
      name: "text",
      source_map: Box::new(source_r_map),
      original_source: Some(inner_source.source().to_string()),
      inner_source_map: inner_source
        .map(&MapOptions::default())
        .map(|source_map| Box::new(source_map) as Box<dyn DecodableSourceMap>),
      remove_original_source: true,
    });
    let expected_content =
      "Translated: Hallo Welt\nist ein test Text\nAnderer Text";
    assert_eq!(sms1.source(), expected_content);
    assert_eq!(sms2.source(), expected_content);
    assert_eq!(
      sms1.map(&MapOptions::default()).unwrap(),
      SourceMap::from_json(
        r#"{
          "mappings": "YAAAA,K,CAAMC;AACN,O,MAAU;ACCC,O,CAAM",
          "names": ["Hello", "World"],
          "sources": ["hello-world.txt", "text"],
          "sourcesContent": [
            "Hello World\nis a test string\n",
            "Hello World\nis a test string\nTranslate: Other text"
          ],
          "version": 3
        }"#
      )
      .unwrap(),
    );
    assert_eq!(
      sms2.map(&MapOptions::default()).unwrap(),
      SourceMap::from_json(
        r#"{
          "mappings": "YAAAA,K,CAAMC;AACN,O,MAAU",
          "names": ["Hello", "World"],
          "sources": ["hello-world.txt"],
          "sourcesContent": ["Hello World\nis a test string\n"],
          "version": 3
        }"#
      )
      .unwrap(),
    );

    let mut hasher = twox_hash::XxHash64::default();
    sms1.hash(&mut hasher);
    assert_eq!(format!("{:x}", hasher.finish()), "d136621583d4618c");
  }

  #[test]
  fn should_handle_null_sources_and_sources_content() {
    let a = SourceMapSource::new(WithoutOriginalOptions {
      value: "hello world\n",
      name: "hello.txt",
      source_map: Box::new(SourceMap::new(
        None,
        Arc::from("AAAA"),
        vec!["".into()],
        vec!["".into()],
        vec![],
      )),
    })
    .boxed();
    let b = SourceMapSource::new(WithoutOriginalOptions {
      value: "hello world\n",
      name: "hello.txt",
      source_map: Box::new(SourceMap::new(
        None,
        Arc::from("AAAA"),
        vec![],
        vec![],
        vec![],
      )),
    })
    .boxed();
    let c = SourceMapSource::new(WithoutOriginalOptions {
      value: "hello world\n",
      name: "hello.txt",
      source_map: Box::new(SourceMap::new(
        None,
        Arc::from("AAAA"),
        vec!["hello-source.txt".into()],
        vec!["hello world\n".into()],
        vec![],
      )),
    })
    .boxed();
    let sources = [a, b, c].into_iter().map(|s| {
      let mut r = ReplaceSource::new(s);
      r.replace(1, 5, "i", None);
      r
    });
    let source = ConcatSource::new(sources);
    assert_eq!(source.source(), "hi world\nhi world\nhi world\n");
    assert_eq!(
      source.map(&MapOptions::default()).unwrap(),
      SourceMap::from_json(
        r#"{
          "mappings": "AAAA;;ACAA,CAAC,CAAI",
          "names": [],
          "sources": [null, "hello-source.txt"],
          "sourcesContent": [null,"hello world\n"],
          "version": 3
        }"#
      )
      .unwrap()
    );
    assert_eq!(
      source.map(&MapOptions::new(false)).unwrap(),
      SourceMap::from_json(
        r#"{
          "mappings": "AAAA;;ACAA",
          "names": [],
          "sources": [null, "hello-source.txt"],
          "sourcesContent": [null,"hello world\n"],
          "version": 3
        }"#
      )
      .unwrap()
    );
  }

  #[test]
  fn should_handle_es6_promise_correctly() {
    let code = include_str!(concat!(
      env!("CARGO_MANIFEST_DIR"),
      "/tests/fixtures/es6-promise.js"
    ));
    let map = SourceMap::from_json(include_str!(concat!(
      env!("CARGO_MANIFEST_DIR"),
      "/tests/fixtures/es6-promise.map"
    )))
    .unwrap()
    .boxed();
    let source = ConcatSource::new([
      SourceMapSource::new(WithoutOriginalOptions {
        value: code,
        name: "es6-promise.js",
        source_map: map.clone(),
      }),
      SourceMapSource::new(WithoutOriginalOptions {
        value: code,
        name: "es6-promise.js",
        source_map: map,
      }),
    ]);
    assert_eq!(source.source(), format!("{code}{code}"));
  }

  #[test]
  fn should_not_emit_zero_sizes_mappings_when_ending_with_empty_mapping() {
    let a = SourceMapSource::new(WithoutOriginalOptions {
      value: "hello\n",
      name: "a",
      source_map: SourceMap::new(
        None,
        Arc::from("AAAA;AACA"),
        vec!["hello1".into()],
        vec![],
        vec![],
      )
      .boxed(),
    })
    .boxed();
    let b = SourceMapSource::new(WithoutOriginalOptions {
      value: "hi",
      name: "b",
      source_map: SourceMap::new(
        None,
        Arc::from("AAAA,EAAE"),
        vec!["hello2".into()],
        vec![],
        vec![],
      )
      .boxed(),
    })
    .boxed();
    let b2 = SourceMapSource::new(WithoutOriginalOptions {
      value: "hi",
      name: "b",
      source_map: SourceMap::new(
        None,
        Arc::from("AAAA,EAAE"),
        vec!["hello3".into()],
        vec![],
        vec![],
      )
      .boxed(),
    })
    .boxed();
    let c = SourceMapSource::new(WithoutOriginalOptions {
      value: "",
      name: "c",
      source_map: SourceMap::new(
        None,
        Arc::from("AAAA"),
        vec!["hello4".into()],
        vec![],
        vec![],
      )
      .boxed(),
    })
    .boxed();
    let source = ConcatSource::new([
      a.clone(),
      a.clone(),
      b.clone(),
      b.clone(),
      b2.clone(),
      b.clone(),
      c.clone(),
      c.clone(),
      b2.clone(),
      a.clone(),
      b2,
      c,
      a,
      b,
    ])
    .boxed();
    let map = source.map(&MapOptions::default()).unwrap().boxed();
    assert_eq!(
      map.mappings(),
      "AAAA;AAAA;ACAA,ICAA,EDAA,ECAA,EFAA;AEAA,EFAA;ACAA",
    );

    macro_rules! test_cached {
      ($s:expr, $fn:expr) => {{
        let c = CachedSource::new($s.clone());
        let o = $fn(&$s);
        let a = $fn(&c);
        assert_eq!(a, o);
        let b = $fn(&c);
        assert_eq!(b, o);
      }};
    }

    test_cached!(source, |s: &dyn Source| s.source().to_string());
    test_cached!(source, |s: &dyn Source| s.map(&MapOptions::default()));
    test_cached!(source, |s: &dyn Source| s.map(&MapOptions {
      columns: false,
      final_source: true
    }));
  }

  #[test]
  fn should_not_crash_without_original_source_when_mapping_names() {
    let source = SourceMapSource::new(SourceMapSourceOptions {
      value: "h",
      name: "hello.txt",
      source_map: SourceMap::from_json(
        r#"{
          "version": 3,
          "sources": ["hello.txt"],
          "mappings": "AAAAA",
          "names": ["hello"]
        }"#,
      )
      .unwrap()
      .boxed(),
      original_source: Some("hello".to_string()),
      inner_source_map: Some(
        SourceMap::from_json(
          r#"{
          "version": 3,
          "sources": ["hello world.txt"],
          "mappings": "AAAA"
        }"#,
        )
        .unwrap()
        .boxed(),
      ),
      remove_original_source: false,
    });
    assert_eq!(
      source.map(&MapOptions::default()).unwrap(),
      SourceMap::from_json(
        r#"{
          "mappings": "AAAA",
          "names": [],
          "sources": ["hello world.txt"],
          "version": 3
        }"#
      )
      .unwrap()
    );
  }

  #[test]
  fn should_map_generated_lines_to_the_inner_source() {
    let source = SourceMapSource::new(SourceMapSourceOptions {
      value: "Message: H W!",
      name: "HELLO_WORLD.txt",
      source_map: SourceMap::from_json(
        r#"{
          "version": 3,
          "sources": ["messages.txt", "HELLO_WORLD.txt"],
          "mappings": "AAAAA,SCAAC,EAAMC,C",
          "names": ["Message", "hello", "world"]
        }"#,
      )
      .unwrap()
      .boxed(),
      original_source: Some("HELLO WORLD".to_string()),
      inner_source_map: Some(
        SourceMap::from_json(
          r#"{
            "version": 3,
            "mappings": "AAAAA,M",
            "sources": ["hello world.txt"],
            "sourcesContent": ["hello world"]
          }"#,
        )
        .unwrap()
        .boxed(),
      ),
      remove_original_source: false,
    });
    assert_eq!(source.source(), "Message: H W!");
    assert_eq!(source.size(), 13);
    assert_eq!(
      source.map(&MapOptions::default()).unwrap(),
      SourceMap::from_json(
        r#"{
          "mappings": "AAAAA,SCAA,ECAMC,C",
          "names": ["Message", "world"],
          "sources": ["messages.txt", "hello world.txt", "HELLO_WORLD.txt"],
          "sourcesContent": [null, "hello world", "HELLO WORLD"],
          "version": 3
        }"#
      )
      .unwrap()
    );
  }

  #[test]
  fn should_map_generated_with_correct_inner_source_index() {
    let source = SourceMapSource::new(SourceMapSourceOptions {
      value: r#"(()=>{function n(){b1("*b0*")}function b(){n("*a0*")}b()})();"#,
      name: "main.js",
      source_map: SourceMap::from_json(
        r#"{
          "version": 3,
          "sources": ["main.js"],
          "mappings": "CAAC,IAAM,CAEL,SAASA,GAAK,CACZ,GAAG,MAAM,CACX,CAGA,SAASC,GAAK,CACZD,EAAG,MAAM,CACX,CACAC,EAAG,CACL,GAAG",
          "names": ["b0", "a0"]
        }"#,
      ).unwrap().boxed(),
      original_source: Some(r#"(() => {
  // b.js
  function b0() {
    b1("*b0*");
  }

  // a.js
  function a0() {
    b0("*a0*");
  }
  a0();
})();
"#.to_string()),
      inner_source_map: Some(SourceMap::from_json(
        r#"{
          "version": 3,
          "sources": ["b.js", "a.js"],
          "sourcesContent": ["export function b0() {\n\tb1(\"*b0*\");\n}\n", "import { b0 } from \"./b.js\";\nfunction a0() {\n\tb0(\"*a0*\");\n}\na0()\n"],
          "mappings": ";;AAAO,WAAS,KAAK;AACpB,OAAG,MAAM;AAAA,EACV;;;ACDA,WAAS,KAAK;AACb,OAAG,MAAM;AAAA,EACV;AACA,KAAG;",
          "names": []
        }"#
      ).unwrap().boxed()),
      remove_original_source: true,
    });
    let map = source.map(&MapOptions::default()).unwrap();
    assert_eq!(
      map,
      SourceMap::from_json(
        r#"{
          "version": 3,
          "sources": ["b.js", "a.js"],
          "sourcesContent": ["export function b0() {\n\tb1(\"*b0*\");\n}\n", "import { b0 } from \"./b.js\";\nfunction a0() {\n\tb0(\"*a0*\");\n}\na0()\n"],
          "names": ["b0", "a0"],
          "mappings": "MAAO,SAASA,GAAK,CACpB,GAAG,MAAM,CACV,CCDA,SAASC,GAAK,CACbD,EAAG,MAAM,CACV,CACAC,EAAG,C"
        }"#
      ).unwrap()
    );
  }

  #[test]
  fn should_have_map_when_columns_is_false_and_last_line_start_is_none() {
    let original = OriginalSource::new("console.log('a')\n", "a.js");
    let source = SourceMapSource::new(WithoutOriginalOptions {
      value: "console.log('a')\n",
      name: "a.js",
      source_map: original.map(&MapOptions::new(false)).unwrap().boxed(),
    });
    let source = ConcatSource::new([
      RawSource::from("\n").boxed(),
      RawSource::from("\n").boxed(),
      RawSource::from("\n").boxed(),
      source.boxed(),
    ]);
    let map = source.map(&MapOptions::new(false)).unwrap();
    assert_eq!(map.mappings().as_ref(), ";;;AAAA");
  }

  #[test]
  fn source_root_is_correctly_applied_to_mappings() {
    let inner_source_code = "Hello World\nis a test string\n";
    let inner_source = ConcatSource::new([
      OriginalSource::new(inner_source_code, "hello-world.txt").boxed(),
      OriginalSource::new("Translate: ", "header.txt").boxed(),
      RawSource::from("Other text").boxed(),
    ]);
    let source_r_code =
      "Translated: Hallo Welt\nist ein test Text\nAnderer Text";
    let source_r_map = SourceMap::from_json(
      r#"{
        "version": 3,
        "sources": [ "text" ],
        "names": [ "Hello", "World", "nope" ],
        "mappings": "YAAAA,K,CAAMC;AACNC,O,MAAU;AACC,O,CAAM",
        "file": "translated.txt",
        "sourcesContent": [ "Hello World\nis a test string\n" ]
      }"#,
    )
    .unwrap()
    .boxed();
    let inner_source_map =
      inner_source.map(&MapOptions::default()).map(|mut map| {
        {
          *map.source_root_mut() = Some(Arc::from("/path/to/folder/"));
          map
        }
        .boxed()
      });
    let sms = SourceMapSource::new(SourceMapSourceOptions {
      value: source_r_code,
      name: "text",
      source_map: source_r_map.clone(),
      original_source: Some(inner_source.source().to_string()),
      inner_source_map,
      remove_original_source: false,
    });
    assert_eq!(
      sms.map(&MapOptions::default()).unwrap(),
      SourceMap::from_json(
        r#"{
          "mappings": "YAAAA,K,CAAMC;AACN,O,MAAU;ACCC,O,CAAM",
          "names": ["Hello", "World"],
          "sources": ["/path/to/folder/hello-world.txt", "text"],
          "sourcesContent": [
            "Hello World\nis a test string\n",
            "Hello World\nis a test string\nTranslate: Other text"
          ],
          "version": 3
        }"#
      )
      .unwrap(),
    );
  }

  #[test]
  fn should_ignores_names_without_columns() {
    let source = SourceMapSource::new(SourceMapSourceOptions {
      value: "h",
      name: "hello.txt",
      source_map: SourceMap::from_json(
        r#"{
          "version": 3,
          "sources": ["hello.txt"],
          "mappings": "AAAAA",
          "names": ["hello"]
        }"#,
      )
      .unwrap()
      .boxed(),
      original_source: Some("hello".to_string()),
      inner_source_map: Some(
        SourceMap::from_json(
          r#"{
          "version": 3,
          "sources": ["hello world.txt"],
          "mappings": "AAAA",
          "names": [],
          "sourcesContent": ["hello, world!"]
        }"#,
        )
        .unwrap()
        .boxed(),
      ),
      remove_original_source: false,
    });
    assert_eq!(
      source.map(&MapOptions::new(false)).unwrap(),
      SourceMap::from_json(
        r#"{
          "mappings": "AAAA",
          "names": [],
          "sources": ["hello world.txt"],
          "version": 3,
          "sourcesContent": ["hello, world!"]
        }"#
      )
      .unwrap()
    );
  }

  #[test]
  fn should_not_panic_when_check_for_an_identity_mapping() {
    let source = SourceMapSource::new(SourceMapSourceOptions {
      value: "hello world",
      name: "hello.txt",
      source_map: SourceMap::from_json(
        r#"{
          "version": 3,
          "sources": ["hello.txt"],
          "mappings": "AAAA,MAAG"
        }"#,
      )
      .unwrap()
      .boxed(),
      original_source: Some("你好 世界".to_string()),
      inner_source_map: Some(
        SourceMap::from_json(
          r#"{
          "version": 3,
          "sources": ["hello world.txt"],
          "mappings": "AAAA,EAAE",
          "sourcesContent": ["你好✋世界"]
        }"#,
        )
        .unwrap()
        .boxed(),
      ),
      remove_original_source: false,
    });
    assert_eq!(
      source.map(&MapOptions::default()).unwrap(),
      SourceMap::from_json(
        r#"{
          "version": 3,
          "mappings": "AAAA,MAAE",
          "sources": ["hello world.txt"],
          "sourcesContent": ["你好✋世界"]
        }"#
      )
      .unwrap()
    );
  }
}
