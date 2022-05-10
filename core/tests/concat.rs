use rspack_sources::{
  CachedSource, ConcatSource, GenMapOption, RawSource, Source, SourceMapSource,
  SourceMapSourceSliceOptions,
};

static FIXTURE_MINIFY: once_cell::sync::Lazy<(Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>)> =
  once_cell::sync::Lazy::new(|| {
    let base_fixure =
      ::std::path::PathBuf::from("tests/fixtures/transpile-minify/files/helloworld");

    let mut original_map_path = base_fixure.clone();
    original_map_path.set_extension("js.map");
    let mut transformed_map_path = base_fixure.clone();
    transformed_map_path.set_extension("min.js.map");

    let mut original_code_path = base_fixure.clone();
    original_code_path.set_extension("js");
    let mut transformed_code_path = base_fixure.clone();
    transformed_code_path.set_extension("min.js");

    let original_map_buf = ::std::fs::read(original_map_path).expect("unable to find test fixture");
    let transformed_map_buf =
      ::std::fs::read(transformed_map_path).expect("unable to find test fixture");
    let original_code_buf =
      ::std::fs::read(original_code_path).expect("unable to find test fixture");
    let transformed_code_buf =
      ::std::fs::read(transformed_code_path).expect("unable to find test fixture");

    (
      original_code_buf,
      original_map_buf,
      transformed_code_buf,
      transformed_map_buf,
    )
  });

static FIXTURE_ROLLUP: once_cell::sync::Lazy<(Vec<u8>, Vec<u8>)> =
  once_cell::sync::Lazy::new(|| {
    let map_buf =
      ::std::fs::read("tests/fixtures/transpile-rollup/files/bundle.js.map").expect("failed");

    let js_buf =
      ::std::fs::read("tests/fixtures/transpile-rollup/files/bundle.js").expect("failed");

    (js_buf, map_buf)
  });

#[test]
fn should_work_with_multiple_source_map_sources() {
  let mut source_map_source_minify =
    SourceMapSource::from_slice(crate::SourceMapSourceSliceOptions {
      source_code: &FIXTURE_MINIFY.2,
      name: "helloworld.min.js".into(),
      source_map: sourcemap::SourceMap::from_slice(&FIXTURE_MINIFY.3).unwrap(),
      original_source: Some(&FIXTURE_MINIFY.0),
      inner_source_map: Some(sourcemap::SourceMap::from_slice(&FIXTURE_MINIFY.1).unwrap()),
      remove_original_source: false,
    })
    .expect("failed");

  let mut source_map_source_rollup =
    SourceMapSource::from_slice(crate::SourceMapSourceSliceOptions {
      source_code: &FIXTURE_ROLLUP.0,
      name: "bundle.js".into(),
      source_map: sourcemap::SourceMap::from_slice(&FIXTURE_ROLLUP.1).unwrap(),
      original_source: None,
      inner_source_map: None,
      remove_original_source: false,
    })
    .expect("failed");

  let mut concat_source = ConcatSource::new(vec![
    &mut source_map_source_rollup,
    &mut source_map_source_minify,
  ]);

  let source_map = concat_source
    .map(&GenMapOption {
      include_source_contents: true,
      file: None,
      columns: true,
    })
    .expect("failed");

  assert_eq!(
    concat_source.source(),
    String::from_utf8(FIXTURE_ROLLUP.0.to_vec()).unwrap()
      + "\n"
      + &String::from_utf8(FIXTURE_MINIFY.2.to_vec()).unwrap()
  );

  let token = source_map.lookup_token(19, 8).expect("should found token");

  assert_eq!(token.get_source(), Some("a.js"));
  assert_eq!(token.get_src_line(), 15);
  assert_eq!(token.get_src_col(), 15);

  println!("rollup source {}", source_map_source_rollup.source());
  println!("end");
  let token = source_map.lookup_token(61, 47).expect("should found token");
  assert_eq!(token.get_source(), Some("helloworld.mjs"));
  assert_eq!(token.get_src_line(), 18);
  assert_eq!(token.get_src_col(), 20);
  assert_eq!(token.get_name(), Some("alert"));
}

#[test]
fn should_work_with_concat_source_map_source_and_cached_source() {
  let mut source_map_source_minify =
    SourceMapSource::from_slice(crate::SourceMapSourceSliceOptions {
      source_code: &FIXTURE_MINIFY.2,
      name: "helloworld.min.js".into(),
      source_map: sourcemap::SourceMap::from_slice(&FIXTURE_MINIFY.3).unwrap(),
      original_source: Some(&FIXTURE_MINIFY.0),
      inner_source_map: Some(sourcemap::SourceMap::from_slice(&FIXTURE_MINIFY.1).unwrap()),
      remove_original_source: false,
    })
    .expect("failed");

  let mut source_map_source_rollup =
    SourceMapSource::from_slice(crate::SourceMapSourceSliceOptions {
      source_code: &FIXTURE_ROLLUP.0,
      name: "bundle.js".into(),
      source_map: sourcemap::SourceMap::from_slice(&FIXTURE_ROLLUP.1).unwrap(),
      original_source: None,
      inner_source_map: None,
      remove_original_source: false,
    })
    .expect("failed");

  let mut concat_source = ConcatSource::new(vec![
    &mut source_map_source_minify,
    &mut source_map_source_rollup,
  ]);

  let concat_source_string = concat_source
    .generate_string(&GenMapOption::default())
    .expect("failed");

  let mut cached_sm_rollup = CachedSource::new(source_map_source_rollup);
  let mut cached_sm_minify: CachedSource<SourceMapSource> = source_map_source_minify.into();

  let mut concat_source_with_cache =
    ConcatSource::new(vec![&mut cached_sm_minify, &mut cached_sm_rollup]);

  let concat_source_with_cache_string = concat_source_with_cache
    .generate_string(&GenMapOption::default())
    .expect("failed");

  assert_eq!(concat_source_string, concat_source_with_cache_string)
}

#[test]
fn should_concat_raw_source() {
  let mut source_map_source_minify =
    SourceMapSource::from_slice(crate::SourceMapSourceSliceOptions {
      source_code: &FIXTURE_MINIFY.2,
      name: "helloworld.min.js".into(),
      source_map: sourcemap::SourceMap::from_slice(&FIXTURE_MINIFY.3).unwrap(),
      original_source: Some(&FIXTURE_MINIFY.0),
      inner_source_map: Some(sourcemap::SourceMap::from_slice(&FIXTURE_MINIFY.1).unwrap()),
      remove_original_source: false,
    })
    .expect("failed");

  let mut raw_source = RawSource::new(r#"console.log("abc")"#.to_owned());

  let mut concat_source = ConcatSource::new(vec![&mut raw_source, &mut source_map_source_minify]);

  assert_eq!(
    concat_source.source(),
    r#"console.log("abc")"#.to_owned()
      + "\n"
      + &String::from_utf8(FIXTURE_MINIFY.2.to_vec()).unwrap()
  );

  let source_map = concat_source.map(&GenMapOption::default()).expect("failed");
  let token = source_map.lookup_token(16, 47).expect("failed");

  assert_eq!(token.get_name(), Some("alert"));
  assert_eq!(token.get_source(), Some("helloworld.mjs"));
  assert_eq!(token.get_src_line(), 18);
  assert_eq!(token.get_src_col(), 20);
}
