use std::{cell::RefCell, marker::PhantomData};

use crate::helpers::SourceText;

#[derive(Debug, Clone)]
pub struct WithIndices<'a, S>
where
  S: SourceText<'a>,
{
  /// line is a string reference
  pub line: S,
  last_char_index_to_byte_index: RefCell<(u32, u32)>,
  data: PhantomData<&'a S>,
}

impl<'a, S> WithIndices<'a, S>
where
  S: SourceText<'a>,
{
  pub fn new(line: S) -> Self {
    Self {
      line,
      last_char_index_to_byte_index: RefCell::new((0, 0)),
      data: PhantomData,
    }
  }

  pub(crate) fn substring(
    &self,
    start_char_index: usize,
    end_char_index: usize,
  ) -> S {
    if end_char_index <= start_char_index {
      return S::default();
    }

    let line_len = self.line.len();

    let mut start_byte_index = None;
    let mut end_byte_index = None;

    let (last_char_index, last_byte_index) =
      *self.last_char_index_to_byte_index.borrow();
    let mut last_byte_index = last_byte_index as usize;
    let mut char_index = last_char_index as usize;
    if start_char_index < last_char_index as usize {
      char_index = 0;
      last_byte_index = 0;
    }
    for (byte_index, _) in self
      .line
      .byte_slice(last_byte_index..line_len)
      .char_indices()
    {
      if char_index == start_char_index {
        start_byte_index = Some(byte_index + last_byte_index);
        if end_char_index == usize::MAX {
          break;
        }
      }
      if char_index == end_char_index {
        end_byte_index = Some(byte_index + last_byte_index);
        *self.last_char_index_to_byte_index.borrow_mut() =
          (end_char_index as u32, (byte_index + last_byte_index) as u32);
        break;
      }
      char_index += 1;
    }

    let start_byte_index = if let Some(start_byte_index) = start_byte_index {
      start_byte_index
    } else {
      return S::default();
    };
    let end_byte_index = end_byte_index.unwrap_or(line_len);

    #[allow(unsafe_code)]
    unsafe {
      // SAFETY: Since `indices` iterates over the `CharIndices` of `self`, we can guarantee
      // that the indices obtained from it will always be within the bounds of `self` and they
      // will always lie on UTF-8 sequence boundaries.
      self
        .line
        .byte_slice_unchecked(start_byte_index..end_byte_index)
    }
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
      WithIndices::new(Rope::from("foobar")).substring(0, 3),
      "foo"
    );
  }

  #[test]
  fn test_out_of_bounds() {
    assert_eq!(
      WithIndices::new(Rope::from("foobar")).substring(0, 10),
      "foobar"
    );
    assert_eq!(WithIndices::new(Rope::from("foobar")).substring(6, 10), "");
  }

  #[test]
  fn test_start_less_than_end() {
    assert_eq!(WithIndices::new(Rope::from("foobar")).substring(3, 2), "");
  }

  #[test]
  fn test_start_and_end_equal() {
    assert_eq!(WithIndices::new(Rope::from("foobar")).substring(3, 3), "");
  }

  #[test]
  fn test_multiple_byte_characters() {
    assert_eq!(
      WithIndices::new(Rope::from("fõøbα®")).substring(2, 5),
      "øbα"
    );
  }

  #[test]
  fn test_last_char_index_to_byte_index() {
    let rope_with_indices = WithIndices::new(Rope::from("foobar"));
    assert_eq!(rope_with_indices.substring(0, 3), "foo");
    assert_eq!(rope_with_indices.substring(3, 6), "bar");
    assert_eq!(rope_with_indices.substring(0, usize::MAX), "foobar");
  }
}
