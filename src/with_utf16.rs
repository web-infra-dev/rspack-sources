use std::cell::OnceCell;

use crate::object_pool::{ObjectPool, Pooled};

#[derive(Debug)]
pub struct WithUtf16<'object_pool, 'text> {
  /// line is a string reference
  pub line: &'text str,
  /// the byte position of each `char` in `line` string slice .
  pub utf16_byte_indices: OnceCell<Pooled<'object_pool>>,
  object_pool: &'object_pool ObjectPool,
}

impl<'object_pool, 'text> WithUtf16<'object_pool, 'text> {
  pub fn new(object_pool: &'object_pool ObjectPool, line: &'text str) -> Self {
    Self {
      utf16_byte_indices: OnceCell::new(),
      line,
      object_pool,
    }
  }

  /// substring::SubString with cache
  pub fn substring(&self, start_index: usize, end_index: usize) -> &'text str {
    if end_index <= start_index {
      return "";
    }

    let utf16_byte_indices = self.utf16_byte_indices.get_or_init(|| {
      let mut vec = self.object_pool.pull(self.line.len());
      for (byte_index, ch) in self.line.char_indices() {
        match ch.len_utf16() {
          1 => vec.push(byte_index),
          2 => {
            vec.push(byte_index);
            vec.push(byte_index);
          }
          _ => unreachable!(),
        }
      }
      vec
    });

    let str_len = self.line.len();
    let start = *utf16_byte_indices.get(start_index).unwrap_or(&str_len);
    let end = *utf16_byte_indices.get(end_index).unwrap_or(&str_len);

    #[allow(unsafe_code)]
    unsafe {
      // SAFETY: Since `indices` iterates over the `CharIndices` of `self`, we can guarantee
      // that the indices obtained from it will always be within the bounds of `self` and they
      // will always lie on UTF-8 sequence boundaries.
      self.line.get_unchecked(start..end)
    }
  }
}

/// tests are just copy from `substring` crate
#[cfg(test)]
mod tests {
  use crate::object_pool::ObjectPool;

  use super::WithUtf16;
  #[test]
  fn test_substring() {
    assert_eq!(
      WithUtf16::new(&ObjectPool::default(), "foobar").substring(0, 3),
      "foo"
    );
  }

  #[test]
  fn test_out_of_bounds() {
    assert_eq!(
      WithUtf16::new(&ObjectPool::default(), "foobar").substring(0, 10),
      "foobar"
    );
    assert_eq!(
      WithUtf16::new(&ObjectPool::default(), "foobar").substring(6, 10),
      ""
    );
  }

  #[test]
  fn test_start_less_than_end() {
    assert_eq!(
      WithUtf16::new(&ObjectPool::default(), "foobar").substring(3, 2),
      ""
    );
  }

  #[test]
  fn test_start_and_end_equal() {
    assert_eq!(
      WithUtf16::new(&ObjectPool::default(), "foobar").substring(3, 3),
      ""
    );
  }

  #[test]
  fn test_multiple_byte_characters() {
    assert_eq!(
      WithUtf16::new(&ObjectPool::default(), "🙈🙉🙊🐒").substring(2, 4),
      "🙉"
    );
  }
}
