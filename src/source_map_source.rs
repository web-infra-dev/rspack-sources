use std::{
  borrow::Cow,
  hash::{Hash, Hasher},
};

use crate::{
  helpers::{
    get_map, stream_chunks_of_combined_source_map, stream_chunks_of_source_map,
    StreamChunks,
  },
  MapOptions, Source, SourceMap,
};

/// Options for [SourceMapSource::new].
#[derive(Debug, Clone, Default)]
pub struct SourceMapSourceOptions<V, N> {
  /// The source code.
  pub value: V,
  /// Name of the file.
  pub name: N,
  /// The source map of the source code.
  pub source_map: SourceMap,
  /// The original source code.
  pub original_source: Option<String>,
  /// The original source map.
  pub inner_source_map: Option<SourceMap>,
  /// Whether remove the original source.
  pub remove_original_source: bool,
}

/// An convenient options for [SourceMapSourceOptions], `original_source` and
/// `inner_source_map` will be `None`, `remove_original_source` will be false.
#[derive(Debug, Clone)]
pub struct WithoutOriginalOptions<V, N> {
  /// The source code.
  pub value: V,
  /// Name of the file.
  pub name: N,
  /// The source map of the source code.
  pub source_map: SourceMap,
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
#[derive(Clone, Eq)]
pub struct SourceMapSource {
  value: String,
  name: String,
  source_map: SourceMap,
  original_source: Option<String>,
  inner_source_map: Option<SourceMap>,
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
      return Some(self.source_map.clone());
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
      && self.source_map == other.source_map
      && self.original_source == other.original_source
      && self.inner_source_map == other.inner_source_map
      && self.remove_original_source == other.remove_original_source
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
    on_chunk: crate::helpers::OnChunk,
    on_source: crate::helpers::OnSource<'_, 'a>,
    on_name: crate::helpers::OnName<'_, 'a>,
  ) -> crate::helpers::GeneratedInfo {
    if let Some(inner_source_map) = &self.inner_source_map {
      stream_chunks_of_combined_source_map(
        &self.value,
        &self.source_map,
        &self.name,
        self.original_source.as_deref(),
        inner_source_map,
        self.remove_original_source,
        on_chunk,
        on_source,
        on_name,
        options,
      )
    } else {
      stream_chunks_of_source_map(
        &self.value,
        &self.source_map,
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
  use crate::{
    CachedSource, ConcatSource, OriginalSource, RawSource, ReplaceSource,
    SourceExt,
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
      source_map: source_r_map.clone(),
      original_source: Some(inner_source.source().to_string()),
      inner_source_map: inner_source.map(&MapOptions::default()),
      remove_original_source: false,
    });
    let sms2 = SourceMapSource::new(SourceMapSourceOptions {
      value: source_r_code,
      name: "text",
      source_map: source_r_map,
      original_source: Some(inner_source.source().to_string()),
      inner_source_map: inner_source.map(&MapOptions::default()),
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
      source_map: SourceMap::new(
        None,
        "AAAA".to_string(),
        vec!["".into()],
        vec!["".into()],
        vec![],
      ),
    });
    let b = SourceMapSource::new(WithoutOriginalOptions {
      value: "hello world\n",
      name: "hello.txt",
      source_map: SourceMap::new(
        None,
        "AAAA".to_string(),
        vec![],
        vec![],
        vec![],
      ),
    });
    let c = SourceMapSource::new(WithoutOriginalOptions {
      value: "hello world\n",
      name: "hello.txt",
      source_map: SourceMap::new(
        None,
        "AAAA".to_string(),
        vec!["hello-source.txt".into()],
        vec!["hello world\n".into()],
        vec![],
      ),
    });
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
    .unwrap();
    let inner = SourceMapSource::new(WithoutOriginalOptions {
      value: code,
      name: "es6-promise.js",
      source_map: map,
    });
    let source = ConcatSource::new([inner.clone(), inner]);
    assert_eq!(source.source(), format!("{code}{code}"));
  }

  #[test]
  fn should_not_emit_zero_sizes_mappings_when_ending_with_empty_mapping() {
    let a = SourceMapSource::new(WithoutOriginalOptions {
      value: "hello\n",
      name: "a",
      source_map: SourceMap::new(
        None,
        "AAAA;AACA".to_string(),
        vec!["hello1".into()],
        vec![],
        vec![],
      ),
    });
    let b = SourceMapSource::new(WithoutOriginalOptions {
      value: "hi",
      name: "b",
      source_map: SourceMap::new(
        None,
        "AAAA,EAAE".to_string(),
        vec!["hello2".into()],
        vec![],
        vec![],
      ),
    });
    let b2 = SourceMapSource::new(WithoutOriginalOptions {
      value: "hi",
      name: "b",
      source_map: SourceMap::new(
        None,
        "AAAA,EAAE".to_string(),
        vec!["hello3".into()],
        vec![],
        vec![],
      ),
    });
    let c = SourceMapSource::new(WithoutOriginalOptions {
      value: "",
      name: "c",
      source_map: SourceMap::new(
        None,
        "AAAA".to_string(),
        vec!["hello4".into()],
        vec![],
        vec![],
      ),
    });
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
    ]);
    let map = source.map(&MapOptions::default()).unwrap();
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
      .unwrap(),
      original_source: Some("hello".to_string()),
      inner_source_map: Some(
        SourceMap::from_json(
          r#"{
          "version": 3,
          "sources": ["hello world.txt"],
          "mappings": "AAAA"
        }"#,
        )
        .unwrap(),
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
      .unwrap(),
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
        .unwrap(),
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
      ).unwrap(),
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
      ).unwrap()),
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
      source_map: original.map(&MapOptions::new(false)).unwrap(),
    });
    let source = ConcatSource::new([
      RawSource::from("\n").boxed(),
      RawSource::from("\n").boxed(),
      RawSource::from("\n").boxed(),
      source.boxed(),
    ]);
    let map = source.map(&MapOptions::new(false)).unwrap();
    assert_eq!(map.mappings(), ";;;AAAA");
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
    .unwrap();
    let inner_source_map =
      inner_source.map(&MapOptions::default()).map(|mut map| {
        map.set_source_root(Some("/path/to/folder/".to_string()));
        map
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
}
