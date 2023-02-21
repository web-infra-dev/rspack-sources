#[derive(Debug, Clone)]
pub struct LineWithIndicesArray<T: AsRef<str>> {
  pub line: T,
  pub prefix_array: Box<[u32]>,
}

impl<T: AsRef<str>> LineWithIndicesArray<T> {
  pub fn new(line: T) -> Self {
    Self {
      prefix_array: line
        .as_ref()
        .char_indices()
        .map(|(i, _)| i as u32)
        .collect::<Vec<_>>()
        .into_boxed_slice(),
      line,
    }
  }

  /// substring::SubString with cache
  #[allow(unsafe_code)]
  pub(crate) fn substring(&self, start_index: usize, end_index: usize) -> &str {
    if end_index <= start_index {
      return "";
    }

    let str_len = self.line.as_ref().len() as u32;
    let start = *self.prefix_array.get(start_index).unwrap_or(&str_len);
    let end = *self.prefix_array.get(end_index).unwrap_or(&str_len);
    unsafe {
      // SAFETY: Since `indices` iterates over the `CharIndices` of `self`, we can guarantee
      // that the indices obtained from it will always be within the bounds of `self` and they
      // will always lie on UTF-8 sequence boundaries.
      self
        .line
        .as_ref()
        .get_unchecked(start as usize..end as usize)
    }
  }
}

/// tests are just copy from `substring` crate
#[cfg(test)]
mod tests {
  use super::LineWithIndicesArray;
  #[test]
  fn test_substring() {
    assert_eq!(LineWithIndicesArray::new("foobar").substring(0, 3), "foo");
  }

  #[test]
  fn test_out_of_bounds() {
    assert_eq!(
      LineWithIndicesArray::new("foobar").substring(0, 10),
      "foobar"
    );
    assert_eq!(LineWithIndicesArray::new("foobar").substring(6, 10), "");
  }

  #[test]
  fn test_start_less_than_end() {
    assert_eq!(LineWithIndicesArray::new("foobar").substring(3, 2), "");
  }

  #[test]
  fn test_start_and_end_equal() {
    assert_eq!(LineWithIndicesArray::new("foobar").substring(3, 3), "");
  }

  #[test]
  fn test_multiple_byte_characters() {
    assert_eq!(LineWithIndicesArray::new("fõøbα®").substring(2, 5), "øbα");
  }
}
