use std::hash::{BuildHasherDefault, Hasher};

#[derive(Default)]
pub struct IdentityHasher(u64);

impl Hasher for IdentityHasher {
  fn finish(&self) -> u64 {
    self.0
  }

  fn write(&mut self, _: &[u8]) {
    panic!("Invalid use of IdentityHasher");
  }

  fn write_u32(&mut self, i: u32) {
    self.0 = i as u64;
  }

  fn write_u64(&mut self, i: u64) {
    self.0 = i;
  }
}

pub type IdentityHashBuilder = BuildHasherDefault<IdentityHasher>;
