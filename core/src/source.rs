use sourcemap::SourceMap;

pub struct GenMapOption {
  /// If set to false the implementation may omit mappings for columns. (default: true)
  pub columns: bool,
}

impl Default for GenMapOption {
  fn default() -> Self {
    Self { columns: true }
  }
}

pub trait Source {
  fn map(&mut self, option: GenMapOption) -> Option<SourceMap>;

  fn source(&self) -> String;
}
