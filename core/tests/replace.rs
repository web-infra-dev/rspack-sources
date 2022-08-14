use std::rc::Rc;

use rspack_sources::{
  GenMapOption, OriginalSource, ReplaceSource, Source, SourceMapSource, SourceMapSourceOptions,
};
use sourcemap::SourceMap;

fn with_readable_mappings(sourcemap: &Rc<SourceMap>) -> String {
  let mut first = true;
  let mut last_line = 0;
  sourcemap
    .tokens()
    .map(|token| {
      format!(
        "{}:{} ->{} {}:{}{}",
        if !first && token.get_dst_line() == last_line {
          ", ".to_owned()
        } else {
          first = false;
          last_line = token.get_dst_line();
          format!("\n{}", token.get_dst_line() + 1)
        },
        token.get_dst_col(),
        token
          .get_source()
          .map_or("".to_owned(), |source| format!(" [{}]", source)),
        token.get_src_line() + 1,
        token.get_src_col(),
        token
          .get_name()
          .map_or("".to_owned(), |source| format!(" ({})", source)),
      )
    })
    .collect()
}

#[test]
fn should_replace_correctly() {
  let line1 = "Hello World!";
  let line2 = "{}";
  let line3 = "Line 3";
  let line4 = "Line 4";
  let line5 = "Line 5";
  let code = [&line1, &line2, &line3, &line4, &line5, "Last", "Line"].join("\n");
  let mut source = ReplaceSource::new(OriginalSource::new(code.as_str(), "file.txt"));

  let start_line3 = (line1.len() + line2.len() + 2) as i32;
  let start_line6 = start_line3 + line3.len() as i32 + line4.len() as i32 + line5.len() as i32 + 3;
  source.replace(start_line3, start_line6, "", None);
  source.replace(1, 5, "i ", None);
  source.replace(1, 5, "bye", None);
  source.replace(7, 8, "0000", None);
  source.insert((line1.len() + 2) as i32, "\n Multi Line\n", None);
  source.replace(start_line6 + 4, start_line6 + 5, " ", None);

  let result = source.source();
  let result_map = source
    .map(&GenMapOption {
      columns: true,
      ..Default::default()
    })
    .expect("replace sources map failed");

  assert_eq!(
    code,
    r#"Hello World!
{}
Line 3
Line 4
Line 5
Last
Line"#
  );

  assert_eq!(
    result,
    r#"Hi bye W0000rld!
{
 Multi Line
}
Last Line"#
  );

  assert_eq!(
    with_readable_mappings(&result_map),
    r#"
1:0 -> [file.txt] 1:0, :1 -> [file.txt] 1:1, :3 -> [file.txt] 1:5, :8 -> [file.txt] 1:7, :12 -> [file.txt] 1:8
2:0 -> [file.txt] 2:0, :1 -> [file.txt] 2:1
3:0 -> [file.txt] 2:1
4:0 -> [file.txt] 2:1, :1 -> [file.txt] 2:2
5:0 -> [file.txt] 6:0, :4 -> [file.txt] 6:4, :5 -> [file.txt] 7:0"#
  );

  let result_list_map = source
    .map(&GenMapOption {
      columns: false,
      ..Default::default()
    })
    .expect("replace sources map failed");
  assert_eq!(
    with_readable_mappings(&result_list_map),
    r#"
1:0 -> [file.txt] 1:0
2:0 -> [file.txt] 2:0
3:0 -> [file.txt] 2:1
4:0 -> [file.txt] 2:1
5:0 -> [file.txt] 6:0"#
  );
}

#[test]
fn should_replace_multiple_items_correctly() {
  let line1 = "Hello";
  let mut source = ReplaceSource::new(OriginalSource::new(
    ["Hello", "World!"].join("\n").as_str(),
    "file.txt",
  ));
  let original_code = source.source();
  source.insert(0, "Message: ", None);
  source.replace(2, (line1.len() + 5) as i32, "y A", None);
  let result_text = source.source();
  let result_map = source
    .map(&GenMapOption {
      columns: true,
      ..Default::default()
    })
    .expect("failed");
  let result_list_map = source
    .map(&GenMapOption {
      columns: false,
      ..Default::default()
    })
    .expect("failed");

  assert_eq!(
    original_code,
    r#"Hello
World!"#
  );
  assert_eq!(result_text, "Message: Hey Ad!");
  assert_eq!(
    with_readable_mappings(&result_map),
    r#"
1:0 -> [file.txt] 1:0, :11 -> [file.txt] 1:2, :14 -> [file.txt] 2:4"#
  );

  assert_eq!(
    with_readable_mappings(&result_list_map),
    r#"
1:0 -> [file.txt] 1:0"#
  );
}

#[test]
fn should_prepend_items_correctly() {
  let mut source = ReplaceSource::new(OriginalSource::new("Line 1", "file.txt"));
  source.insert(-1, "Line -1\n", None);
  source.insert(-1, "Line 0\n", None);

  let result_text = source.source();
  let result_map = source
    .map(&GenMapOption {
      columns: true,
      ..Default::default()
    })
    .expect("failed");
  let result_list_map = source
    .map(&GenMapOption {
      columns: false,
      ..Default::default()
    })
    .expect("failed");

  assert_eq!(result_text, "Line -1\nLine 0\nLine 1");
  assert_eq!(
    with_readable_mappings(&result_map),
    r#"
1:0 -> [file.txt] 1:0
2:0 -> [file.txt] 1:0
3:0 -> [file.txt] 1:0"#
  );
  assert_eq!(
    with_readable_mappings(&result_list_map),
    r#"
1:0 -> [file.txt] 1:0
2:0 -> [file.txt] 1:0
3:0 -> [file.txt] 1:0"#
  );
}

#[test]
fn should_prepend_items_with_replace_at_start_correctly() {
  let mut source = ReplaceSource::new(OriginalSource::new(
    ["Line 1", "Line 2"].join("\n").as_str(),
    "file.txt",
  ));
  source.insert(-1, "Line 0\n", None);
  source.replace(0, 6, "Hello", None);
  let result_text = source.source();
  let result_map = source
    .map(&GenMapOption {
      columns: true,
      ..Default::default()
    })
    .expect("failed");
  let result_list_map = source
    .map(&GenMapOption {
      columns: false,
      ..Default::default()
    })
    .expect("failed");

  assert_eq!(
    result_text,
    r#"Line 0
Hello
Line 2"#
  );

  let mut writer = vec![];
  result_map.to_writer(&mut writer).expect("failed");

  assert_eq!(
    String::from_utf8(writer).unwrap(),
    r#"{"version":3,"sources":["file.txt"],"sourcesContent":["Line 1\nLine 2"],"names":[],"mappings":"AAAA;AAAA,KAAM;AACN"}"#
  );

  let mut writer = vec![];
  result_list_map.to_writer(&mut writer).expect("failed");

  assert_eq!(
    String::from_utf8(writer).unwrap(),
    r#"{"version":3,"sources":["file.txt"],"sourcesContent":["Line 1\nLine 2"],"names":[],"mappings":"AAAA;AAAA;AACA"}"#
  );
}

#[test]
fn should_append_items_correctly() {
  let line1 = "Line 1\n";
  let mut source = ReplaceSource::new(OriginalSource::new(line1, "file.txt"));
  source.insert((line1.len() + 1) as i32, "Line 2\n", None);
  let result_text = source.source();
  let result_map = source
    .map(&GenMapOption {
      columns: true,
      ..Default::default()
    })
    .expect("failed");
  let result_list_map = source
    .map(&GenMapOption {
      columns: false,
      ..Default::default()
    })
    .expect("failed");

  assert_eq!(result_text, "Line 1\nLine 2\n");

  let mut writer = vec![];
  result_map.to_writer(&mut writer).expect("failed");
  assert_eq!(
    String::from_utf8(writer).unwrap(),
    r#"{"version":3,"sources":["file.txt"],"sourcesContent":["Line 1\n"],"names":[],"mappings":"AAAA"}"#
  );

  let mut writer = vec![];
  result_list_map.to_writer(&mut writer).expect("failed");

  assert_eq!(
    String::from_utf8(writer).unwrap(),
    r#"{"version":3,"sources":["file.txt"],"sourcesContent":["Line 1\n"],"names":[],"mappings":"AAAA"}"#
  );
}

#[test]
fn should_produce_correct_source_map() {
  let bootstrap_code = "   var hello\n   var world\n";
  let mut source = ReplaceSource::new(OriginalSource::new(bootstrap_code, "file.js"));
  source.replace(7, 12, "h", Some("hello"));
  source.replace(20, 25, "w", Some("world"));
  let result_map = source
    .map(&GenMapOption {
      ..Default::default()
    })
    .expect("failed");

  let target_code = source.source();
  assert_eq!(target_code, "   var h\n   var w\n");

  assert_eq!(
    with_readable_mappings(&result_map),
    r#"
1:0 -> [file.js] 1:0, :7 -> [file.js] 1:7 (hello), :8 -> [file.js] 1:12
2:0 -> [file.js] 2:0, :7 -> [file.js] 2:7 (world), :8 -> [file.js] 2:12"#
  );

  let mut writer = vec![];
  result_map.to_writer(&mut writer).expect("failed");
  assert_eq!(
    String::from_utf8(writer).unwrap(),
    r#"{"version":3,"sources":["file.js"],"sourcesContent":["   var hello\n   var world\n"],"names":["hello","world"],"mappings":"AAAA,OAAOA,CAAK;AACZ,OAAOC,CAAK"}"#
  );
}

#[test]
fn should_allow_replacements_at_the_start() {
  let map = SourceMap::from_slice(
    r#"{
      "version":3,
      "sources":["abc"],
      "names":["StaticPage","data","foo"],
      "mappings":";;AAAA,eAAe,SAASA,UAAT,OAA8B;AAAA,MAARC,IAAQ,QAARA,IAAQ;AAC3C,sBAAO;AAAA,cAAMA,IAAI,CAACC;AAAX,IAAP;AACD",
      "sourcesContent":["export default function StaticPage({ data }) {\nreturn <div>{data.foo}</div>\n}\n"],
      "file":"x"
    }"#.as_bytes(),
  ).expect("failed");

  let code = r#"import { jsx as _jsx } from "react/jsx-runtime";
export var __N_SSG = true;
export default function StaticPage(_ref) {
  var data = _ref.data;
  return /*#__PURE__*/_jsx("div", {
    children: data.foo
  });
}"#;

  /*
    3:0 -> [abc] 1:0, :15 -> [abc] 1:15, :24 -> [abc] 1:24 (StaticPage), :34 -> [abc] 1:15, :41 -> [abc] 1:45
    4:0 -> [abc] 1:45, :6 -> [abc] 1:37 (data), :10 -> [abc] 1:45, :18 -> [abc] 1:37 (data), :22 -> [abc] 1:45
    5:0 -> [abc] 2:2, :22 -> [abc] 2:9
    6:0 -> [abc] 2:9, :14 -> [abc] 2:15 (data), :18 -> [abc] 2:19, :19 -> [abc] 2:20 (foo)
    7:0 -> [abc] 2:9, :4 -> [abc] 2:2
    8:0 -> [abc] 3:1
  */

  let mut source = ReplaceSource::new(SourceMapSource::new(SourceMapSourceOptions {
    source_code: code.to_string(),
    name: "source.js".to_string(),
    source_map: map,
    original_source: None,
    inner_source_map: None,
    remove_original_source: false,
  }));
  source.replace(0, 48, "", None);
  source.replace(49, 56, "", None);
  source.replace(76, 91, "", None);
  source.replace(
    165,
    169,
    "(0,react_jsx_runtime__WEBPACK_IMPORTED_MODULE_0__.jsx)",
    None,
  );

  let target_code = source.source();
  let source_map = source
    .map(&GenMapOption {
      ..Default::default()
    })
    .expect("failed");

  assert_eq!(
    target_code,
    r#"
var __N_SSG = true;
function StaticPage(_ref) {
  var data = _ref.data;
  return /*#__PURE__*/(0,react_jsx_runtime__WEBPACK_IMPORTED_MODULE_0__.jsx)("div", {
    children: data.foo
  });
}"#
  );
  assert_eq!(source_map.get_name(0).unwrap(), "StaticPage");
  assert_eq!(source_map.get_name(1).unwrap(), "data");
  assert_eq!(source_map.get_name(2).unwrap(), "foo");
  assert_eq!(
    source_map.get_source_contents(0).unwrap(),
    r#"export default function StaticPage({ data }) {
return <div>{data.foo}</div>
}
"#
  );
  assert_eq!(source_map.get_file().unwrap(), "x");
  assert_eq!(source_map.get_source(0).unwrap(), "abc");

  assert_eq!(
    with_readable_mappings(&source_map),
    r#"
3:0 -> [abc] 1:15, :9 -> [abc] 1:24 (StaticPage), :19 -> [abc] 1:15, :26 -> [abc] 1:45
4:0 -> [abc] 1:45, :6 -> [abc] 1:37 (data), :10 -> [abc] 1:45, :18 -> [abc] 1:37 (data), :22 -> [abc] 1:45
5:0 -> [abc] 2:2, :22 -> [abc] 2:9
6:0 -> [abc] 2:9, :14 -> [abc] 2:15 (data), :18 -> [abc] 2:19, :19 -> [abc] 2:20 (foo)
7:0 -> [abc] 2:9, :4 -> [abc] 2:2
8:0 -> [abc] 3:1"#
  );
}

#[test]
fn should_not_generate_invalid_mappings_when_replacing_mulitple_lines_of_code() {
  let mut source = ReplaceSource::new(OriginalSource::new(
    r#"if (a;b;c) {
  a; b; c;
}"#,
    "document.js",
  ));
  source.replace(4, 9, "false", None);
  source.replace(12, 24, "", None);

  let target_code = source.source();
  let source_map = source
    .map(&GenMapOption {
      ..Default::default()
    })
    .expect("failed");

  assert_eq!(target_code, "if (false) {}");
  assert_eq!(
    with_readable_mappings(&source_map),
    r#"
1:0 -> [document.js] 1:0, :4 -> [document.js] 1:4, :9 -> [document.js] 1:9, :11 -> [document.js] 1:11, :12 -> [document.js] 3:0"#
  );

  let mut writer = vec![];
  source_map.to_writer(&mut writer).expect("failed");
  assert_eq!(
    String::from_utf8(writer).unwrap(),
    r#"{"version":3,"sources":["document.js"],"sourcesContent":["if (a;b;c) {\n  a; b; c;\n}"],"names":[],"mappings":"AAAA,IAAI,KAAK,EAAE,CAEX"}"#
  );
}

#[test]
fn test_edge_case() {
  let line1 = "hello world\n";
  let mut source = ReplaceSource::new(OriginalSource::new(line1, "file.txt"));

  source.replace(-1, -999, "start2\n", None);
  source.insert(-2, "start1\n", None);
  source.replace(999, 10000, "end2", None);
  source.insert(888, "end1\n", None);
  source.replace(-1, 999, "replaced!\n", Some("whole"));

  let result_text = source.source();
  let result_map = source
    .map(&GenMapOption {
      columns: true,
      ..Default::default()
    })
    .expect("failed");

  assert_eq!(result_text, "start1\nstart2\nreplaced!\nend1\nend2");

  assert_eq!(
    with_readable_mappings(&result_map),
    r#"
1:0 -> [file.txt] 1:0
2:0 -> [file.txt] 1:0
3:0 -> [file.txt] 1:0 (whole)"#
  );
}
