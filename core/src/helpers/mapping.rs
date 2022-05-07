// use rayon::prelude::*;
// use sourcemap::vlq::parse_vlq_segment;
//
// fn decode_mapping(mapping: &str) {
//   let mut mapping: Vec<Vec<Vec<i64>>> = Default::default();
//
//   let lines = mapping
//     .split(";")
//     .map(|c| c.to_owned())
//     .collect::<Vec<String>>();
//
//   lines.into_iter().for_each(|line| {
//     let segments = line.split(",").map(|c| c.to_owned()).collect::<Vec<_>>();
//   })
// }
