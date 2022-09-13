#![feature(test)]
#![allow(soft_unstable)]

extern crate test;
use test::Bencher;

use rspack_sources::{CachedSource, ConcatSource, MapOptions, SourceMapSource};

const helloworld_js: &'static str = include_str!(concat!(
  env!("CARGO_MANIFEST_DIR"),
  "/benches/fixtures/transpile-minify/files/helloworld.js"
));
const helloworld_js_map: &'static str = include_str!(concat!(
  env!("CARGO_MANIFEST_DIR"),
  "/benches/fixtures/transpile-minify/files/helloworld.js.map"
));
const helloworld_min_js: &'static str = include_str!(concat!(
  env!("CARGO_MANIFEST_DIR"),
  "/benches/fixtures/transpile-minify/files/helloworld.min.js"
));
const helloworld_min_js_map: &'static str = include_str!(concat!(
  env!("CARGO_MANIFEST_DIR"),
  "/benches/fixtures/transpile-minify/files/helloworld.min.js.map"
));

// static FIXTURE_ROLLUP: once_cell::sync::Lazy<(Vec<u8>, Vec<u8>)> =
//   once_cell::sync::Lazy::new(|| {
//     let map_buf =
//       ::std::fs::read("tests/fixtures/transpile-rollup/files/bundle.js.map").expect("failed");

//     let js_buf =
//       ::std::fs::read("tests/fixtures/transpile-rollup/files/bundle.js").expect("failed");

//     (js_buf, map_buf)
//   });

// #[bench]
// fn benchmark_concat_generate_string(b: &mut Bencher) {
//   let mut source_map_source_minify =
//     SourceMapSource::from_slice(rspack_sources::SourceMapSourceSliceOptions {
//       source_code: &FIXTURE_MINIFY.2,
//       name: "helloworld.min.js".into(),
//       source_map: sourcemap::SourceMap::from_slice(&FIXTURE_MINIFY.3).unwrap(),
//       original_source: Some(&FIXTURE_MINIFY.0),
//       inner_source_map: Some(sourcemap::SourceMap::from_slice(&FIXTURE_MINIFY.1).unwrap()),
//       remove_original_source: false,
//     })
//     .expect("failed");

//   let mut source_map_source_rollup =
//     SourceMapSource::from_slice(rspack_sources::SourceMapSourceSliceOptions {
//       source_code: &FIXTURE_ROLLUP.0,
//       name: "bundle.js".into(),
//       source_map: sourcemap::SourceMap::from_slice(&FIXTURE_ROLLUP.1).unwrap(),
//       original_source: None,
//       inner_source_map: None,
//       remove_original_source: false,
//     })
//     .expect("failed");

//   b.iter(|| {
//     // for _ in 0..100 {
//     let mut concat_source = ConcatSource::new(vec![
//       &mut source_map_source_rollup,
//       &mut source_map_source_minify,
//     ]);

//     concat_source
//       .generate_string(&GenMapOption {
//         columns: true,
//         include_source_contents: true,
//         file: None,
//       })
//       .expect("failed");
//     // }
//   })
// }

// #[bench]
// fn benchmark_concat_generate_string_with_cache(b: &mut Bencher) {
//   let source_map_source_minify =
//     SourceMapSource::new(SourceMapSourceSliceOptions {
//       source_code: &FIXTURE_MINIFY.2,
//       name: "helloworld.min.js".into(),
//       source_map: sourcemap::SourceMap::from_slice(&FIXTURE_MINIFY.3).unwrap(),
//       original_source: Some(&FIXTURE_MINIFY.0),
//       inner_source_map: Some(sourcemap::SourceMap::from_slice(&FIXTURE_MINIFY.1).unwrap()),
//       remove_original_source: false,
//     })
//     .expect("failed");

//   let source_map_source_rollup =
//     SourceMapSource::from_slice(rspack_sources::SourceMapSourceSliceOptions {
//       source_code: &FIXTURE_ROLLUP.0,
//       name: "bundle.js".into(),
//       source_map: sourcemap::SourceMap::from_slice(&FIXTURE_ROLLUP.1).unwrap(),
//       original_source: None,
//       inner_source_map: None,
//       remove_original_source: false,
//     })
//     .expect("failed");

//   let mut cached_rollup = CachedSource::new(source_map_source_rollup);
//   let mut cached_minify = CachedSource::new(source_map_source_minify);

//   b.iter(|| {
//     let mut concat_source = ConcatSource::new(vec![&mut cached_rollup, &mut cached_minify]);
//     concat_source
//       .generate_string(&GenMapOption {
//         columns: true,
//         include_source_contents: true,
//         file: None,
//       });
//   })
// }

// #[bench]
// fn benchmark_concat_generate_base64(b: &mut Bencher) {
//   let mut source_map_source_minify =
//     SourceMapSource::from_slice(rspack_sources::SourceMapSourceSliceOptions {
//       source_code: &FIXTURE_MINIFY.2,
//       name: "helloworld.min.js".into(),
//       source_map: sourcemap::SourceMap::from_slice(&FIXTURE_MINIFY.3).unwrap(),
//       original_source: Some(&FIXTURE_MINIFY.0),
//       inner_source_map: Some(sourcemap::SourceMap::from_slice(&FIXTURE_MINIFY.1).unwrap()),
//       remove_original_source: false,
//     })
//     .expect("failed");

//   let mut source_map_source_rollup =
//     SourceMapSource::from_slice(rspack_sources::SourceMapSourceSliceOptions {
//       source_code: &FIXTURE_ROLLUP.0,
//       name: "bundle.js".into(),
//       source_map: sourcemap::SourceMap::from_slice(&FIXTURE_ROLLUP.1).unwrap(),
//       original_source: None,
//       inner_source_map: None,
//       remove_original_source: false,
//     })
//     .expect("failed");

//   b.iter(|| {
//     // for _ in 0..100 {
//     let mut concat_source = ConcatSource::new(vec![
//       &mut source_map_source_rollup,
//       &mut source_map_source_minify,
//     ]);
//     concat_source
//       .generate_base64(&GenMapOption {
//         columns: true,
//         include_source_contents: true,
//         file: None,
//       })
//       .expect("failed");
//     // }
//   })
// }

// #[bench]
// fn benchmark_concat_generate_base64_with_cache(b: &mut Bencher) {
//   let source_map_source_minify =
//     SourceMapSource::from_slice(rspack_sources::SourceMapSourceSliceOptions {
//       source_code: &FIXTURE_MINIFY.2,
//       name: "helloworld.min.js".into(),
//       source_map: sourcemap::SourceMap::from_slice(&FIXTURE_MINIFY.3).unwrap(),
//       original_source: Some(&FIXTURE_MINIFY.0),
//       inner_source_map: Some(sourcemap::SourceMap::from_slice(&FIXTURE_MINIFY.1).unwrap()),
//       remove_original_source: false,
//     })
//     .expect("failed");

//   let source_map_source_rollup =
//     SourceMapSource::from_slice(rspack_sources::SourceMapSourceSliceOptions {
//       source_code: &FIXTURE_ROLLUP.0,
//       name: "bundle.js".into(),
//       source_map: sourcemap::SourceMap::from_slice(&FIXTURE_ROLLUP.1).unwrap(),
//       original_source: None,
//       inner_source_map: None,
//       remove_original_source: false,
//     })
//     .expect("failed");

//   let mut cached_rollup = CachedSource::new(source_map_source_rollup);
//   let mut cached_minify = CachedSource::new(source_map_source_minify);

//   b.iter(|| {
//     // for _ in 0..100 {
//     let mut concat_source = ConcatSource::new(vec![&mut cached_rollup, &mut cached_minify]);
//     concat_source
//       .generate_base64(&GenMapOption {
//         columns: true,
//         include_source_contents: true,
//         file: None,
//       })
//       .expect("failed");
//     // }
//   })
// }

// #[bench]
// fn benchmark_concat_generate_url(b: &mut Bencher) {
//   let mut source_map_source_minify =
//     SourceMapSource::from_slice(rspack_sources::SourceMapSourceSliceOptions {
//       source_code: &FIXTURE_MINIFY.2,
//       name: "helloworld.min.js".into(),
//       source_map: sourcemap::SourceMap::from_slice(&FIXTURE_MINIFY.3).unwrap(),
//       original_source: Some(&FIXTURE_MINIFY.0),
//       inner_source_map: Some(sourcemap::SourceMap::from_slice(&FIXTURE_MINIFY.1).unwrap()),
//       remove_original_source: false,
//     })
//     .expect("failed");

//   let mut source_map_source_rollup =
//     SourceMapSource::from_slice(rspack_sources::SourceMapSourceSliceOptions {
//       source_code: &FIXTURE_ROLLUP.0,
//       name: "bundle.js".into(),
//       source_map: sourcemap::SourceMap::from_slice(&FIXTURE_ROLLUP.1).unwrap(),
//       original_source: None,
//       inner_source_map: None,
//       remove_original_source: false,
//     })
//     .expect("failed");

//   b.iter(|| {
//     // for _ in 0..100 {
//     let mut concat_source = ConcatSource::new(vec![
//       &mut source_map_source_rollup,
//       &mut source_map_source_minify,
//     ]);
//     concat_source
//       .generate_url(&GenMapOption {
//         columns: true,
//         include_source_contents: true,
//         file: None,
//       })
//       .expect("failed");
//     // }
//   })
// }

// #[bench]
// fn benchmark_concat_generate_url_with_cache(b: &mut Bencher) {
//   let source_map_source_minify =
//     SourceMapSource::from_slice(rspack_sources::SourceMapSourceSliceOptions {
//       source_code: &FIXTURE_MINIFY.2,
//       name: "helloworld.min.js".into(),
//       source_map: sourcemap::SourceMap::from_slice(&FIXTURE_MINIFY.3).unwrap(),
//       original_source: Some(&FIXTURE_MINIFY.0),
//       inner_source_map: Some(sourcemap::SourceMap::from_slice(&FIXTURE_MINIFY.1).unwrap()),
//       remove_original_source: false,
//     })
//     .expect("failed");

//   let source_map_source_rollup =
//     SourceMapSource::from_slice(rspack_sources::SourceMapSourceSliceOptions {
//       source_code: &FIXTURE_ROLLUP.0,
//       name: "bundle.js".into(),
//       source_map: sourcemap::SourceMap::from_slice(&FIXTURE_ROLLUP.1).unwrap(),
//       original_source: None,
//       inner_source_map: None,
//       remove_original_source: false,
//     })
//     .expect("failed");

//   let mut cached_rollup = CachedSource::new(source_map_source_rollup);
//   let mut cached_minify = CachedSource::new(source_map_source_minify);

//   b.iter(|| {
//     // for _ in 0..100 {
//     let mut concat_source = ConcatSource::new(vec![&mut cached_rollup, &mut cached_minify]);
//     concat_source
//       .generate_url(&GenMapOption {
//         columns: true,
//         include_source_contents: true,
//         file: None,
//       })
//       .expect("failed");
//     // }
//   })
// }
