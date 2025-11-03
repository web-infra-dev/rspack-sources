use std::sync::Arc;

use crate::{
  helpers::split_into_lines, object_pool::ObjectPool, with_indices::WithIndices,
};

struct Owner<'object_pool> {
  text: Arc<str>,
  object_pool: &'object_pool ObjectPool,
}

type BorrowedValue<'text> = Vec<WithIndices<'text, &'text str>>;

self_cell::self_cell!(
  pub struct SourceContentLines<'object_pool> {
    owner: Owner<'object_pool>,
    #[covariant]
    dependent: BorrowedValue,
  }
);

impl<'object_pool> SourceContentLines<'object_pool> {
  pub fn get(&self, line: usize) -> Option<&WithIndices<'_, &str>> {
    self.borrow_dependent().get(line)
  }

  pub fn from(object_pool: &'object_pool ObjectPool, text: Arc<str>) -> Self {
    let owner = Owner {
      text,
      object_pool,
    };

    SourceContentLines::new(owner, |owner| {
      split_into_lines(&owner.text.as_ref())
        .map(|line| WithIndices::new(owner.object_pool, line))
        .collect::<Vec<_>>()
    })
  }
}
