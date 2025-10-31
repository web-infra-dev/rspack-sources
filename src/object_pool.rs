use std::{
  cell::RefCell,
  collections::BTreeMap,
  rc::Rc,
  sync::{Arc, LazyLock},
  thread::ThreadId,
};

use dashmap::DashMap;

// Vector pooling minimum capacity threshold
// Recommended threshold: 64
// Reasons:
// 1. Memory consideration: 64 * 8 bytes = 512 bytes, a reasonable memory block size
// 2. Allocation cost: Allocations smaller than 512 bytes are usually fast, pooling benefits are limited
// 3. Cache friendly: 512 bytes can typically utilize CPU cache well
// 4. Empirical value: 64 is a proven balance point in real projects
const MIN_POOL_CAPACITY: usize = 64;

pub trait Poolable {
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
  objects: Rc<RefCell<BTreeMap<usize, Vec<T>>>>,
}

// SAFETY: Each ObjectPool is only used within a single thread in rspack-sources,
// which is guaranteed by THREAD_ISOLATED_MAP. Therefore, it is safe to implement Send and Sync.
#[allow(unsafe_code)]
unsafe impl<T> Send for ObjectPool<T> {}
#[allow(unsafe_code)]
unsafe impl<T> Sync for ObjectPool<T> {}

impl<T> Clone for ObjectPool<T> {
  fn clone(&self) -> Self {
    Self {
      objects: self.objects.clone(),
    }
  }
}

impl<T: Poolable> ObjectPool<T> {
  /// Retrieves a reusable `T` from the pool with at least the requested capacity.
  pub fn pull(&self, requested_capacity: usize) -> T {
    if requested_capacity < MIN_POOL_CAPACITY
      || self.objects.borrow().is_empty()
    {
      return T::with_capacity(requested_capacity);
    }
    let mut objects = self.objects.borrow_mut();
    if let Some((_, bucket)) = objects.range_mut(requested_capacity..).next() {
      if let Some(mut object) = bucket.pop() {
        object.clear();
        return object;
      }
    }
    T::with_capacity(requested_capacity)
  }

  /// Returns a `T` to the pool for future reuse.
  pub fn return_to_pool(&self, object: T) {
    if object.capacity() < MIN_POOL_CAPACITY {
      return;
    }
    let mut objects = self.objects.borrow_mut();
    let cap = object.capacity();
    let bucket = objects.entry(cap).or_default();
    bucket.push(object);
  }

  pub fn clear(&self) {
    self.objects.borrow_mut().clear();
  }
}

/// A smart pointer that holds a pooled object and automatically returns it to the pool when dropped.
///
/// `Pooled<T>` implements RAII (Resource Acquisition Is Initialization) pattern to manage
/// pooled objects lifecycle. When the `Pooled` instance is dropped, the contained object
/// is automatically returned to its associated pool for future reuse.
#[derive(Debug)]
pub struct Pooled<T: Poolable> {
  object: Option<T>,
  pool: Option<ObjectPool<T>>,
}

impl<T: Poolable> Pooled<T> {
  pub fn new(pool: Option<ObjectPool<T>>, requested_capacity: usize) -> Self {
    let object = match &pool {
      Some(pool) => pool.pull(requested_capacity),
      None => T::with_capacity(requested_capacity),
    };
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
      if let Some(pool) = &self.pool {
        pool.return_to_pool(object);
      }
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

pub static THREAD_ISOLATED_MAP: LazyLock<
  Arc<DashMap<ThreadId, ObjectPool<Vec<usize>>>>,
> = LazyLock::new(|| Arc::new(DashMap::new()));

/// Cleans up the object pool when not in pooling mode to prevent memory retention.
pub fn clear_current_thread_object_pool() {
  let thread_id = std::thread::current().id();
  if let Some(thread_isolated_map) = THREAD_ISOLATED_MAP.get(&thread_id) {
    thread_isolated_map.value().clear();
  }
}

pub fn pull_usize_vec(requested_capacity: usize) -> Pooled<Vec<usize>> {
  let thread_id = std::thread::current().id();
  let pool = THREAD_ISOLATED_MAP.entry(thread_id).or_default();
  Pooled::new(Some(pool.clone()), requested_capacity)
}
