use std::{cell::OnceCell, cmp::min};

#[derive(Debug, Clone)]
pub struct WithIndices<T: AsRef<str>> {
  /// line is a string reference
  pub line: T,
  pub is_ascii: bool,
  /// the byte position of each `char` in `line` string slice .
  pub indices_indexes: OnceCell<Vec<usize>>,
}

impl<T: AsRef<str>> WithIndices<T> {
  pub fn new(line: T) -> Self {
    let is_ascii = line.as_ref().is_ascii();
    Self {
      line,
      is_ascii,
      indices_indexes: OnceCell::new(),
    }
  }

  /// substring::SubString with cache
  #[allow(unsafe_code)]
  pub(crate) fn substring(&self, start_index: usize, end_index: usize) -> &str {
    if end_index <= start_index {
      return "";
    }

    let line = self.line.as_ref();
    let line_len = line.len();
    let (start, end) = if self.is_ascii {
      let start = min(start_index, line_len);
      let end = min(end_index, line_len);
      (start, end)
    } else {
      let indices_indexes = self
        .indices_indexes
        .get_or_init(|| {
          line.char_indices().map(|(i, _)| i).collect::<Vec<_>>()
        });
      let start = *indices_indexes.get(start_index).unwrap_or(&line_len);
      let end = *indices_indexes.get(end_index).unwrap_or(&line_len);
      (start, end)
    };

    unsafe {
      // SAFETY: Since `indices` iterates over the `CharIndices` of `self`, we can guarantee
      // that the indices obtained from it will always be within the bounds of `self` and they
      // will always lie on UTF-8 sequence boundaries.
      line.get_unchecked(start..end)
    }
  }
}

/// tests are just copy from `substring` crate
#[cfg(test)]
mod tests {
  use super::WithIndices;
  #[test]
  fn test_substring() {
    assert_eq!(WithIndices::new("foobar").substring(0, 3), "foo");
  }

  #[test]
  fn test_out_of_bounds() {
    assert_eq!(WithIndices::new("foobar").substring(0, 10), "foobar");
    assert_eq!(WithIndices::new("foobar").substring(6, 10), "");
  }

  #[test]
  fn test_start_less_than_end() {
    assert_eq!(WithIndices::new("foobar").substring(3, 2), "");
  }

  #[test]
  fn test_start_and_end_equal() {
    assert_eq!(WithIndices::new("foobar").substring(3, 3), "");
  }

  #[test]
  fn test_multiple_byte_characters() {
    assert_eq!(WithIndices::new("fõøbα®").substring(2, 5), "øbα");
  }
}
