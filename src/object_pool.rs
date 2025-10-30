use std::{
  cell::{OnceCell, RefCell},
  collections::BTreeMap,
  rc::Rc,
  sync::atomic::AtomicBool,
};

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

thread_local! {
  pub static USIZE_VEC_POOL: OnceCell<ObjectPool<Vec<usize>>> = OnceCell::default();
}

pub static IN_USING_OBJECT_POOL: AtomicBool = AtomicBool::new(false);

/// Executes a function with object pooling enabled for the current thread.
///
/// This function temporarily enables a thread-local object pool for `Vec<usize>` allocations,
/// executes the provided closure, and then cleans up the pool to prevent memory leaks.
pub fn using_object_pool<F, R>(f: F) -> R
where
  F: FnOnce() -> R,
{
  IN_USING_OBJECT_POOL.store(true, std::sync::atomic::Ordering::Relaxed);
  // Initialize the thread-local pool if needed
  USIZE_VEC_POOL.with(|once_cell| {
    once_cell.get_or_init(ObjectPool::default);
  });

  let result = f();

  IN_USING_OBJECT_POOL.store(false, std::sync::atomic::Ordering::Relaxed);
  USIZE_VEC_POOL.with(|once_cell| {
    if let Some(pool) = once_cell.get() {
      pool.clear();
    }
  });

  result
}

/// Cleans up the object pool when not in pooling mode to prevent memory retention.
///
/// This function is called automatically after map operations complete to ensure
/// that memory is not retained unnecessarily outside of pooling contexts.
pub fn cleanup_idle_object_pool() {
  // Only clear if we're not in an explicit pooling context
  if !IN_USING_OBJECT_POOL.load(std::sync::atomic::Ordering::Relaxed) {
    USIZE_VEC_POOL.with(|once_cell| {
      if let Some(pool) = once_cell.get() {
        pool.clear();
      }
    });
  }
}
