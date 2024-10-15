use std::cell::OnceCell;

#[derive(Debug, Clone)]
pub struct WithIndices<'a> {
  /// line is a string reference
  pub line: &'a str,
  /// the byte position of each `char` in `line` string slice .
  pub indices_indexes: OnceCell<Vec<usize>>,
}

impl<'a> WithIndices<'a> {
  pub fn new(line: &'a str) -> Self {
    Self {
      indices_indexes: OnceCell::new(),
      line,
    }
  }

  /// substring::SubString with cache
  #[allow(unsafe_code)]
  pub(crate) fn substring(&self, start_index: usize, end_index: usize) -> &'a str {
    if end_index <= start_index {
      return "";
    }

    let indices_indexes = self
      .indices_indexes
      .get_or_init(|| self.line.char_indices().map(|(i, _)| i).collect::<Vec<_>>());

    let str_len = self.line.len();
    let start = *indices_indexes.get(start_index).unwrap_or(&str_len);
    let end = *indices_indexes.get(end_index).unwrap_or(&str_len);
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
