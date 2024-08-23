//! Implements utilities for dealing with the sourcemap vlq encoding.
//! forked from [rust-sourcemap](https://github.com/getsentry/rust-sourcemap/blob/851f12bfa6c4cf2c737b94734b27f7d9bfb4de86/src/vlq.rs)

use arrayvec::ArrayVec;

use crate::{Error, Result};

const B64_CHARS: &[u8] =
  b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const B64: [i8; 256] = [
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  62,
  -1,
  -1,
  -1,
  63,
  52,
  53,
  54,
  55,
  56,
  57,
  58,
  59,
  60,
  61,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  0,
  1,
  2,
  3,
  4,
  5,
  6,
  7,
  8,
  9,
  10,
  11,
  12,
  13,
  14,
  15,
  16,
  17,
  18,
  19,
  20,
  21,
  22,
  23,
  24,
  25,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  26,
  27,
  28,
  29,
  30,
  31,
  32,
  33,
  34,
  35,
  36,
  37,
  38,
  39,
  40,
  41,
  42,
  43,
  44,
  45,
  46,
  47,
  48,
  49,
  50,
  51,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
  -1,
];

/// Parses a VLQ segment into a pre-allocated `Vec` instead of returning a new allocation.
pub fn decode(segment: &[u8], rv: &mut ArrayVec<i64, 5>) -> Result<()> {
  let mut cur = 0;
  let mut shift = 0;

  for c in segment {
    let enc = i64::from(B64[*c as usize]);
    let val = enc & 0b11111;
    let cont = enc >> 5;
    cur += val.checked_shl(shift).ok_or(Error::VlqOverflow)?;
    shift += 5;

    if cont == 0 {
      let sign = cur & 1;
      cur >>= 1;
      if sign != 0 {
        cur = -cur;
      }
      rv.push(cur);
      cur = 0;
      shift = 0;
    }
  }

  if cur != 0 || shift != 0 {
    Err(Error::VlqLeftover)
  } else if rv.is_empty() {
    Err(Error::VlqNoValues)
  } else {
    Ok(())
  }
}

fn encode_vlq(out: &mut String, num: i64) {
  let mut num = if num < 0 { ((-num) << 1) + 1 } else { num << 1 };

  loop {
    let mut digit = num & 0b11111;
    num >>= 5;
    if num > 0 {
      digit |= 1 << 5;
    }
    out.push(B64_CHARS[digit as usize] as char);
    if num == 0 {
      break;
    }
  }
}

pub fn encode(out: &mut String, a: u32, b: u32) {
  encode_vlq(out, i64::from(a) - i64::from(b))
}
