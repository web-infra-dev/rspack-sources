//! Implements utilities for dealing with the sourcemap vlq encoding.
//! forked from [rust-sourcemap](https://github.com/getsentry/rust-sourcemap/blob/851f12bfa6c4cf2c737b94734b27f7d9bfb4de86/src/vlq.rs)

use crate::{Error, Result};

const B64_CHARS: &[u8] =
  b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const B64: [i8; 256] = [
  -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
  -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
  -1, -1, -1, -1, -1, 62, -1, -1, -1, 63, 52, 53, 54, 55, 56, 57, 58, 59, 60,
  61, -1, -1, -1, -1, -1, -1, -1, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13,
  14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, -1, -1, -1, -1, -1, -1, 26,
  27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45,
  46, 47, 48, 49, 50, 51, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
  -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
  -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
  -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
  -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
  -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
  -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
  -1, -1, -1, -1, -1, -1,
];

pub struct VlqIter<'a> {
  segment: std::slice::Iter<'a, u8>,
  cur: i64,
  shift: u32,
}

impl<'a> Iterator for VlqIter<'a> {
  type Item = Result<i64>;

  fn next(&mut self) -> Option<Self::Item> {
    loop {
      match self.segment.next() {
        Some(c) => {
          let enc = i64::from(B64[*c as usize]);
          let val = enc & 0b11111;
          let cont = enc >> 5;
          self.cur +=
            match val.checked_shl(self.shift).ok_or(Error::VlqOverflow) {
              Ok(v) => v,
              Err(e) => return Some(Err(e)),
            };
          self.shift += 5;

          if cont == 0 {
            let sign = self.cur & 1;
            self.cur >>= 1;
            if sign != 0 {
              self.cur = -self.cur;
            }
            let result = self.cur;
            self.cur = 0;
            self.shift = 0;
            return Some(Ok(result));
          }
        }
        None => {
          if self.cur != 0 || self.shift != 0 {
            return Some(Err(Error::VlqLeftover));
          } else {
            return None;
          }
        }
      }
    }
  }
}

pub fn decode(segment: &[u8]) -> VlqIter {
  VlqIter {
    segment: segment.iter(),
    cur: 0,
    shift: 0,
  }
}

pub fn encode(out: &mut Vec<u8>, a: u32, b: u32) {
  let mut num = if a >= b {
    (a - b) << 1
  } else {
    ((b - a) << 1) + 1
  };

  loop {
    let mut digit = num & 0b11111;
    num >>= 5;
    if num > 0 {
      digit |= 1 << 5;
    }
    out.push(B64_CHARS[digit as usize]);
    if num == 0 {
      break;
    }
  }
}
