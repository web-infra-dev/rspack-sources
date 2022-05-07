use sourcemap::SourceMap;

pub struct GenMapOption {
  // @default true
  columns: bool,
}

pub trait Source {
  fn map(&mut self, option: GenMapOption) -> Option<SourceMap>;

  fn source(&mut self) -> Vec<u8>;
}
