use smol_str::SmolStr;
use std::collections::HashMap;

use sourcemap::SourceMap;

use crate::source::GenMapOption;
use crate::Source;

pub struct CachedSource<T: Source> {
  inner: T,
  cached_map: HashMap<GenMapOption, Option<SourceMap>>,
  cached_code: Option<SmolStr>,
}

unsafe impl<T: Source + Sync> Sync for CachedSource<T> {}
unsafe impl<T: Source + Send> Send for CachedSource<T> {}

impl<T: Source> CachedSource<T> {
  pub fn new(source: T) -> Self {
    Self {
      inner: source,
      cached_code: Default::default(),
      cached_map: Default::default(),
    }
  }

  pub fn into_inner(self) -> T {
    self.inner
  }
}

impl<T: Source> Source for CachedSource<T> {
  #[inline]
  fn map(&mut self, gen_map_option: &GenMapOption) -> Option<SourceMap> {
    use std::collections::hash_map::Entry;

    return match self.cached_map.entry(gen_map_option.clone()) {
      Entry::Occupied(record) => record.get().clone(),
      Entry::Vacant(v) => {
        let map = self.inner.map(gen_map_option);
        v.insert(map.clone());
        map
      }
    };
  }

  #[inline]
  fn source(&mut self) -> SmolStr {
    if let Some(cached_code) = &self.cached_code {
      return cached_code.clone();
    }
    let code = self.inner.source();
    self.cached_code = Some(code.clone());
    code
  }
}
