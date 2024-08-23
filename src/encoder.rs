use crate::{vlq::encode, Mapping};

pub(crate) trait MappingsEncoder {
  fn encode(&mut self, mapping: &Mapping);
  fn drain(self: Box<Self>) -> String;
}

pub fn create_encoder(columns: bool) -> Box<dyn MappingsEncoder> {
  if columns {
    Box::new(FullMappingsEncoder::new())
  } else {
    Box::new(LinesOnlyMappingsEncoder::new())
  }
}

struct FullMappingsEncoder {
  current_line: u32,
  current_column: u32,
  current_original_line: u32,
  current_original_column: u32,
  current_source_index: u32,
  current_name_index: u32,
  active_mapping: bool,
  active_name: bool,
  initial: bool,
  mappings: String,
}

impl FullMappingsEncoder {
  pub fn new() -> Self {
    Self {
      current_line: 1,
      current_column: 0,
      current_original_line: 1,
      current_original_column: 0,
      current_source_index: 0,
      current_name_index: 0,
      active_mapping: false,
      active_name: false,
      initial: true,
      mappings: Default::default(),
    }
  }
}

impl MappingsEncoder for FullMappingsEncoder {
  fn encode(&mut self, mapping: &Mapping) {
    if self.active_mapping && self.current_line == mapping.generated_line {
      // A mapping is still active
      if mapping.original.is_some_and(|original| {
        original.source_index == self.current_source_index
          && original.original_line == self.current_original_line
          && original.original_column == self.current_original_column
          && !self.active_name
          && original.name_index.is_none()
      }) {
        // avoid repeating the same original mapping
        return;
      }
    } else {
      // No mapping is active
      if mapping.original.is_none() {
        // avoid writing unnecessary generated mappings
        return;
      }
    }

    if self.current_line < mapping.generated_line {
      (0..mapping.generated_line - self.current_line)
        .for_each(|_| self.mappings.push(';'));
      self.current_line = mapping.generated_line;
      self.current_column = 0;
      self.initial = false;
    } else if self.initial {
      self.initial = false;
    } else {
      self.mappings.push(',');
    }

    encode(
      &mut self.mappings,
      mapping.generated_column,
      self.current_column,
    );
    self.current_column = mapping.generated_column;
    if let Some(original) = &mapping.original {
      self.active_mapping = true;
      if original.source_index == self.current_source_index {
        self.mappings.push('A');
      } else {
        encode(
          &mut self.mappings,
          original.source_index,
          self.current_source_index,
        );
        self.current_source_index = original.source_index;
      }
      encode(
        &mut self.mappings,
        original.original_line,
        self.current_original_line,
      );
      self.current_original_line = original.original_line;
      if original.original_column == self.current_original_column {
        self.mappings.push('A');
      } else {
        encode(
          &mut self.mappings,
          original.original_column,
          self.current_original_column,
        );
        self.current_original_column = original.original_column;
      }
      if let Some(name_index) = original.name_index {
        encode(&mut self.mappings, name_index, self.current_name_index);
        self.current_name_index = name_index;
        self.active_name = true;
      } else {
        self.active_name = false;
      }
    } else {
      self.active_mapping = false;
    }
  }

  fn drain(self: Box<Self>) -> String {
    self.mappings
  }
}

pub(crate) struct LinesOnlyMappingsEncoder {
  last_written_line: u32,
  current_line: u32,
  current_source_index: u32,
  current_original_line: u32,
  mappings: String,
}

impl LinesOnlyMappingsEncoder {
  pub fn new() -> Self {
    Self {
      last_written_line: 0,
      current_line: 1,
      current_source_index: 0,
      current_original_line: 1,
      mappings: Default::default(),
    }
  }
}

impl MappingsEncoder for LinesOnlyMappingsEncoder {
  fn encode(&mut self, mapping: &Mapping) {
    if let Some(original) = &mapping.original {
      if self.last_written_line == mapping.generated_line {
        // avoid writing multiple original mappings per line
        return;
      }
      self.last_written_line = mapping.generated_line;
      if mapping.generated_line == self.current_line + 1 {
        self.current_line = mapping.generated_line;
        if original.source_index == self.current_source_index {
          if original.original_line == self.current_original_line + 1 {
            self.current_original_line = original.original_line;
            self.mappings.push_str(";AACA");
          } else {
            self.mappings.push_str(";AA");
            encode(
              &mut self.mappings,
              original.original_line,
              self.current_original_line,
            );
            self.current_original_line = original.original_line;
            self.mappings.push('A');
          }
        } else {
          self.mappings.push_str(";A");
          encode(
            &mut self.mappings,
            original.source_index,
            self.current_source_index,
          );
          self.current_source_index = original.source_index;
          encode(
            &mut self.mappings,
            original.original_line,
            self.current_original_line,
          );
          self.current_original_line = original.original_line;
          self.mappings.push('A');
        }
      } else {
        (0..mapping.generated_line - self.current_line)
          .for_each(|_| self.mappings.push(';'));
        self.current_line = mapping.generated_line;
        if original.source_index == self.current_source_index {
          if original.original_line == self.current_original_line + 1 {
            self.current_original_line = original.original_line;
            self.mappings.push_str("AACA");
          } else {
            self.mappings.push_str("AA");
            encode(
              &mut self.mappings,
              original.original_line,
              self.current_original_line,
            );
            self.current_original_line = original.original_line;
            self.mappings.push('A');
          }
        } else {
          self.mappings.push('A');
          encode(
            &mut self.mappings,
            original.source_index,
            self.current_source_index,
          );
          self.current_source_index = original.source_index;
          encode(
            &mut self.mappings,
            original.original_line,
            self.current_original_line,
          );
          self.current_original_line = original.original_line;
          self.mappings.push('A');
        }
      }
    }
  }

  fn drain(self: Box<Self>) -> String {
    self.mappings
  }
}
