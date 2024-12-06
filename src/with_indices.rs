use std::cell::OnceCell;

use crate::Rope;

#[derive(Debug, Clone)]
pub struct WithIndices<'a> {
  /// line is a string reference
  pub line: Rope<'a>,
  /// the byte position of each `char` in `line` string slice .
  pub indices_indexes: OnceCell<Vec<usize>>,
}

impl<'a> WithIndices<'a> {
  pub fn new(line: Rope<'a>) -> Self {
    Self {
      indices_indexes: OnceCell::new(),
      line,
    }
  }

  /// substring::SubString with cache
  pub(crate) fn substring(
    &self,
    start_index: usize,
    end_index: usize,
  ) -> Rope<'a> {
    if end_index <= start_index {
      return Rope::default();
    }

    let indices_indexes = self.indices_indexes.get_or_init(|| {
      self.line.char_indices().map(|(i, _)| i).collect::<Vec<_>>()
    });

    let str_len = self.line.len();
    let start = *indices_indexes.get(start_index).unwrap_or(&str_len);
    let end = *indices_indexes.get(end_index).unwrap_or(&str_len);
    self.line.byte_slice(start..end)
  }
}

/// tests are just copy from `substring` crate
#[cfg(test)]
mod tests {
  use crate::Rope;

  use super::WithIndices;
  #[test]
  fn test_substring() {
    assert_eq!(
      WithIndices::new(Rope::from_str("foobar")).substring(0, 3),
      "foo"
    );
  }

  #[test]
  fn test_out_of_bounds() {
    assert_eq!(
      WithIndices::new(Rope::from_str("foobar")).substring(0, 10),
      "foobar"
    );
    assert_eq!(
      WithIndices::new(Rope::from_str("foobar")).substring(6, 10),
      ""
    );
  }

  #[test]
  fn test_start_less_than_end() {
    assert_eq!(
      WithIndices::new(Rope::from_str("foobar")).substring(3, 2),
      ""
    );
  }

  #[test]
  fn test_start_and_end_equal() {
    assert_eq!(
      WithIndices::new(Rope::from_str("foobar")).substring(3, 3),
      ""
    );
  }

  #[test]
  fn test_multiple_byte_characters() {
    assert_eq!(
      WithIndices::new(Rope::from_str("fõøbα®")).substring(2, 5),
      "øbα"
    );
  }
}
