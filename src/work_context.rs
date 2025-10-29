use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

const MIN_POOL_CAPACITY: usize = 64;

#[derive(Default, Debug)]
pub struct WorkContext {
  usize_vec_pool: RefCell<BTreeMap<usize, Vec<Vec<usize>>>>,
}

impl WorkContext {
  pub fn new() -> Self {
    Self {
      usize_vec_pool: RefCell::new(BTreeMap::new()),
    }
  }

  pub fn pull_usize_vec(&self, requested_capacity: usize) -> Vec<usize> {
    if requested_capacity < MIN_POOL_CAPACITY {
      return Vec::with_capacity(requested_capacity);
    }
    let mut usize_vec_pool = self.usize_vec_pool.borrow_mut();
    if let Some((_, bucket)) = usize_vec_pool.range_mut(requested_capacity..).next() {
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
pub struct PooledVec {
  vec: Option<Vec<usize>>,
  context: Rc<WorkContext>,
}

impl PooledVec {
  pub fn new(context: Rc<WorkContext>, requested_capacity: usize) -> Self {
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

impl Drop for PooledVec {
  fn drop(&mut self) {
    if let Some(vec) = self.vec.take() {
      self.context.return_usize_vec(vec);
    }
  }
}

impl std::ops::Deref for PooledVec {
  type Target = Vec<usize>;

  fn deref(&self) -> &Self::Target {
    self.as_ref()
  }
}

impl std::ops::DerefMut for PooledVec {
  fn deref_mut(&mut self) -> &mut Self::Target {
    self.as_mut()
  }
}
