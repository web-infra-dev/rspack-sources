#![feature(slice_internals)]
use core::slice::memchr::memchr;
use std::time::Instant;

use memchr::memmem;
use rspack_sources::helpers::SegmentIter;
fn main() {
  let source = r#"AAAA,eAAe,SAASA,UAAT,OAA8B;AAAA,MAARC,IAAQ,QAARA,IAAQ;AAC3C,sBAAO;AAAA,cAAMA,IAAI,CAACC;AAAX,IAAP;AACD"#;


  let start = Instant::now();
  for i in 0..100 {
    let _  = source.find(';');
    // dbg!(&res);
  }
  dbg!(&start.elapsed());

  let start = Instant::now();
  for i in 0..100 {
    let _  = memchr(b';', source.as_bytes());
    // dbg!(&res);
  }

  dbg!(&start.elapsed());

  let start = Instant::now();
  for i in 0..100 {
    let _ = memchr::memchr(b';', source.as_bytes());
    // dbg!(&res);
  }
  dbg!(&start.elapsed());
  //   let ret = SegmentIter {
  //     line: "",
  //     // cursor: 0,
  //     mapping_str: source,
  //     source_index: 0,
  //     original_line: 1,
  //     original_column: 0,
  //     name_index: 0,
  //     generated_line: 0,
  //     segment_cursor: 0,
  //     generated_column: 0,
  //   };
  //   for r in ret {
  //     // dbg!(&r);
  //   }
}
