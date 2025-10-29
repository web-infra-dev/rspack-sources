use std::{cell::RefCell, collections::BTreeMap};

// Vector pooling minimum capacity threshold
// Recommended threshold: 64
// Reasons:
// 1. Memory consideration: 64 * 8 bytes = 512 bytes, a reasonable memory block size
// 2. Allocation cost: Allocations smaller than 512 bytes are usually fast, pooling benefits are limited
// 3. Cache friendly: 512 bytes can typically utilize CPU cache well
// 4. Empirical value: 64 is a proven balance point in real projects
const MIN_POOL_CAPACITY: usize = 64;

#[derive(Default, Debug)]
pub struct WorkContext {
  usize_vec_pool: RefCell<BTreeMap<usize, Vec<Vec<usize>>>>,
}

impl WorkContext {
  pub fn pull_usize_vec(&self, requested_capacity: usize) -> Vec<usize> {
    if requested_capacity < MIN_POOL_CAPACITY
      || self.usize_vec_pool.borrow().len() == 0
    {
      return Vec::with_capacity(requested_capacity);
    }
    let mut usize_vec_pool = self.usize_vec_pool.borrow_mut();
    if let Some((_, bucket)) =
      usize_vec_pool.range_mut(requested_capacity..).next()
    {
      if let Some(mut v) = bucket.pop() {
        v.clear();
        return v;
      }
    }
    Vec::with_capacity(requested_capacity)
  }

  pub fn return_usize_vec(&self, vec: Vec<usize>) {
    if vec.capacity() < MIN_POOL_CAPACITY {
      return;
    }
    let mut usize_vec_pool = self.usize_vec_pool.borrow_mut();
    let cap = vec.capacity();
    let bucket = usize_vec_pool.entry(cap).or_default();
    bucket.push(vec);
  }
}

#[derive(Debug)]
pub struct PooledUsizeVec<'a> {
  vec: Option<Vec<usize>>,
  context: &'a WorkContext,
}

impl<'a> PooledUsizeVec<'a> {
  pub fn new(context: &'a WorkContext, requested_capacity: usize) -> Self {
    let vec = context.pull_usize_vec(requested_capacity);
    Self {
      vec: Some(vec),
      context,
    }
  }

  pub fn as_mut(&mut self) -> &mut Vec<usize> {
    self.vec.as_mut().unwrap()
  }

  pub fn as_ref(&self) -> &Vec<usize> {
    self.vec.as_ref().unwrap()
  }
}

impl Drop for PooledUsizeVec<'_> {
  fn drop(&mut self) {
    if let Some(vec) = self.vec.take() {
      self.context.return_usize_vec(vec);
    }
  }
}

impl std::ops::Deref for PooledUsizeVec<'_> {
  type Target = Vec<usize>;

  fn deref(&self) -> &Self::Target {
    self.as_ref()
  }
}

impl std::ops::DerefMut for PooledUsizeVec<'_> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    self.as_mut()
  }
}
