use std::sync::Arc;

use crate::{
  helpers::split_into_lines, object_pool::ObjectPool, with_indices::WithIndices,
};

pub struct SourceContentLines<'object_pool> {
  text: Arc<str>,
  // Self-referential data structure: lines borrow from the text.
  lines: Vec<WithIndices<'object_pool, 'static, &'static str>>,
}

impl<'object_pool> SourceContentLines<'object_pool> {
  pub fn new(object_pool: &'object_pool ObjectPool, text: Arc<str>) -> Self {
    // SAFETY: We extend the lifetime of the &str to 'static because the Arc<str> is owned by this struct,
    // and all &'static str references are only used within the lifetime of this struct.
    #[allow(unsafe_code)]
    let text_ref =
      unsafe { std::mem::transmute::<&str, &'static str>(text.as_ref()) };
    let lines = split_into_lines::<&str>(&text_ref)
      .map(|line| WithIndices::new(object_pool, line))
      .collect::<Vec<_>>();
    Self { text, lines }
  }

  pub fn get(
    &self,
    line: usize,
  ) -> Option<&WithIndices<'object_pool, '_, &str>> {
    let _ = &self.text;
    self.lines.get(line)
  }
}
