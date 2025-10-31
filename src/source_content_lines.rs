use std::sync::Arc;

use crate::{helpers::split_into_lines, with_indices::WithIndices};

type Owner = Arc<str>;

type BorrowedValue<'a> = Vec<WithIndices<'a, &'a str>>;

self_cell::self_cell!(
  pub struct SourceContentLines {
    owner: Owner,
    #[covariant]
    dependent: BorrowedValue,
  }
);

impl SourceContentLines {
  pub fn get(&self, line: usize) -> Option<&WithIndices<'_, &str>> {
    self.borrow_dependent().get(line)
  }
}

impl From<Arc<str>> for SourceContentLines {
  fn from(value: Arc<str>) -> Self {
    SourceContentLines::new(value, |owner| {
      split_into_lines(&owner.as_ref())
        .map(WithIndices::new)
        .collect::<Vec<_>>()
    })
  }
}
