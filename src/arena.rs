use std::ops::{Deref, DerefMut};

use bumpalo::Bump;

#[derive(Default)]
#[allow(missing_docs)]
pub struct Arena {
  bump: Bump,
}

impl Deref for Arena {
  type Target = Bump;

  fn deref(&self) -> &Self::Target {
    &self.bump
  }
}

impl DerefMut for Arena {
  fn deref_mut(&mut self) -> &mut Self::Target {
    &mut self.bump
  }
}
