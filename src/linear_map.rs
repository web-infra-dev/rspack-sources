use bumpalo::{collections::Vec, Bump};

pub struct LinearMap<'a, V: Default> {
  inner: Vec<'a, V>,
}

impl<'a, V: Default> LinearMap<'a, V> {
  pub fn new(bump: &'a Bump) -> Self {
    Self {
      inner: Vec::new_in(bump),
    }
  }

  pub fn get(&self, key: &u32) -> Option<&V> {
    self.inner.get(*key as usize)
  }

  pub fn insert(&mut self, key: u32, value: V) {
    let key = key as usize;
    while key >= self.inner.len() {
      self.inner.push(Default::default());
    }
    self.inner[key] = value;
  }

  pub fn clear(&mut self) {
    self.inner.clear()
  }
}
