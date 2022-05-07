use sourcemap::SourceMap;

use crate::{source::{Source, GenMapOption}, helpers::get_map};


pub struct ConcatSource {
    children: Vec<&dyn Source>
}

impl ConcatSource {
    fn add(&mut self, item: &dyn Source) {
        self.children.append(item);
    }
}

impl Source for ConcatSource {
    fn source(&mut self) -> Vec<u8> {
      let mut source = vec![];
      self.children.iter().for_each(|child| source.append(&mut child.source()));
      source
    }
  
    fn map(&mut self, option: GenMapOption) -> Option<SourceMap> {
       get_map(option)
    }
}