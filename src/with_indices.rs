use std::{cell::OnceCell, marker::PhantomData, rc::Rc};

use crate::{helpers::SourceText, work_context::{PooledVec, WorkContext}};

#[derive(Debug)]
pub struct WithIndices<'a, S>
where
  S: SourceText<'a>,
{
  /// line is a string reference
  pub line: S,
  /// the byte position of each `char` in `line` string slice .
  pub indices_indexes: OnceCell<PooledVec>,
  work_context: Rc<WorkContext>,
  data: PhantomData<&'a S>,
}

impl<'a, S> WithIndices<'a, S>
where
  S: SourceText<'a>,
{
  pub fn new(work_context: Rc<WorkContext>, line: S) -> Self {
    Self {
      indices_indexes: OnceCell::new(),
      line,
      work_context,
      data: PhantomData,
    }
  }

  /// substring::SubString with cache
  pub(crate) fn substring(&self, start_index: usize, end_index: usize) -> S {
    if end_index <= start_index {
      return S::default();
    }

    let indices_indexes = &*self.indices_indexes.get_or_init(|| {
      let mut vec = PooledVec::new(self.work_context.clone(), self.line.len());
      for (i, _) in self.line.char_indices() {
        vec.push(i);
      }
      vec
    });

    let str_len = self.line.len();
    let start = *indices_indexes.get(start_index).unwrap_or(&str_len);
    let end = *indices_indexes.get(end_index).unwrap_or(&str_len);

    #[allow(unsafe_code)]
    unsafe {
      // SAFETY: Since `indices` iterates over the `CharIndices` of `self`, we can guarantee
      // that the indices obtained from it will always be within the bounds of `self` and they
      // will always lie on UTF-8 sequence boundaries.
      self.line.byte_slice_unchecked(start..end)
    }
  }
}

/// tests are just copy from `substring` crate
#[cfg(test)]
mod tests {
  use std::rc::Rc;

use crate::{work_context::WorkContext, Rope};

  use super::WithIndices;
  #[test]
  fn test_substring() {
    assert_eq!(
      WithIndices::new(Rc::new(WorkContext::default()), Rope::from("foobar")).substring(0, 3),
      "foo"
    );
  }

  #[test]
  fn test_out_of_bounds() {
    assert_eq!(
      WithIndices::new(Rc::new(WorkContext::default()), Rope::from("foobar")).substring(0, 10),
      "foobar"
    );
    assert_eq!(WithIndices::new(Rc::new(WorkContext::default()), Rope::from("foobar")).substring(6, 10), "");
  }

  #[test]
  fn test_start_less_than_end() {
    assert_eq!(WithIndices::new(Rc::new(WorkContext::default()), Rope::from("foobar")).substring(3, 2), "");
  }

  #[test]
  fn test_start_and_end_equal() {
    assert_eq!(WithIndices::new(Rc::new(WorkContext::default()), Rope::from("foobar")).substring(3, 3), "");
  }

  #[test]
  fn test_multiple_byte_characters() {
    assert_eq!(
      WithIndices::new(Rc::new(WorkContext::default()), Rope::from("fõøbα®")).substring(2, 5),
      "øbα"
    );
  }
}
