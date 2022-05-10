use std::collections::btree_map::Entry;
use std::collections::HashMap;

use sourcemap::SourceMap;

use crate::source::GenMapOption;
use crate::Source;

pub struct CachedSource<T: Source> {
  pub(crate) inner: T,
  cached_map: HashMap<GenMapOption, Option<SourceMap>>,
  cached_code: Option<String>,
}

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

  fn source(&mut self) -> String {
    if self.cached_code.is_some() {
      return self.cached_code.clone().unwrap();
    }
    let code = self.inner.source();
    self.cached_code = Some(code.clone());
    code
  }
}
