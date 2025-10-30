use std::{
  cell::RefCell, collections::BTreeMap, rc::Rc, sync::atomic::AtomicBool,
};

// Vector pooling minimum capacity threshold
// Recommended threshold: 64
// Reasons:
// 1. Memory consideration: 64 * 8 bytes = 512 bytes, a reasonable memory block size
// 2. Allocation cost: Allocations smaller than 512 bytes are usually fast, pooling benefits are limited
// 3. Cache friendly: 512 bytes can typically utilize CPU cache well
// 4. Empirical value: 64 is a proven balance point in real projects
const MIN_POOL_CAPACITY: usize = 64;

trait Poolable {
  fn with_capacity(capacity: usize) -> Self;
  fn capacity(&self) -> usize;
  fn clear(&mut self);
}

impl<T> Poolable for Vec<T> {
  fn with_capacity(capacity: usize) -> Self {
    Vec::with_capacity(capacity)
  }

  fn capacity(&self) -> usize {
    self.capacity()
  }

  fn clear(&mut self) {
    self.clear();
  }
}

/// A memory pool for reusing `T` allocations to reduce memory allocation overhead.
#[derive(Default, Debug)]
pub struct ObjectPool<T> {
  usize_vec_pool: RefCell<BTreeMap<usize, Vec<T>>>,
}

impl<T: Poolable> ObjectPool<T> {
  /// Retrieves a reusable `T` from the pool with at least the requested capacity.
  pub fn pull(&self, requested_capacity: usize) -> T {
    if requested_capacity < MIN_POOL_CAPACITY
      || self.usize_vec_pool.borrow().is_empty()
    {
      return T::with_capacity(requested_capacity);
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
    T::with_capacity(requested_capacity)
  }

  /// Returns a `T` to the pool for future reuse.
  pub fn ret(&self, object: T) {
    if object.capacity() < MIN_POOL_CAPACITY {
      return;
    }
    let mut usize_vec_pool = self.usize_vec_pool.borrow_mut();
    let cap = object.capacity();
    let bucket = usize_vec_pool.entry(cap).or_default();
    bucket.push(object);
  }
}

#[derive(Debug)]
pub struct Pooled<T: Poolable> {
  object: Option<T>,
  pool: Rc<ObjectPool<T>>,
}

impl<T: Poolable> Pooled<T> {
  fn new(pool: Rc<ObjectPool<T>>, requested_capacity: usize) -> Self {
    let object = pool.pull(requested_capacity);
    Self {
      object: Some(object),
      pool,
    }
  }

  pub fn as_mut(&mut self) -> &mut T {
    self.object.as_mut().unwrap()
  }

  pub fn as_ref(&self) -> &T {
    self.object.as_ref().unwrap()
  }
}

impl<T: Poolable> Drop for Pooled<T> {
  fn drop(&mut self) {
    if let Some(object) = self.object.take() {
      self.pool.ret(object);
    }
  }
}

impl<T: Poolable> std::ops::Deref for Pooled<T> {
  type Target = T;

  fn deref(&self) -> &Self::Target {
    self.as_ref()
  }
}

impl<T: Poolable> std::ops::DerefMut for Pooled<T> {
  fn deref_mut(&mut self) -> &mut Self::Target {
    self.as_mut()
  }
}

pub(crate) const USING_OBJECT_POOL: AtomicBool = AtomicBool::new(false);

pub fn using_object_pool<F, R>(f: F) -> R
where
  F: FnOnce() -> R,
{
  USING_OBJECT_POOL.store(true, std::sync::atomic::Ordering::SeqCst);
  let result = f();
  USING_OBJECT_POOL.store(false, std::sync::atomic::Ordering::SeqCst);
  result
}
