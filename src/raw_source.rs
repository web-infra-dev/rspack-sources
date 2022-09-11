use std::borrow::Cow;

use crate::{
  helpers::{
    get_generated_source_info, split_into_lines, GeneratedInfo, OnChunk,
    OnName, OnSource, StreamChunks,
  },
  source::Mapping,
  MapOptions, Source, SourceMap,
};

// impl Source for String {
//   fn source(&self) -> Cow<str> {
//     Cow::Borrowed(self)
//   }

//   fn buffer(&self) -> Cow<[u8]> {
//     Cow::Borrowed(self.as_bytes())
//   }

//   fn size(&self) -> usize {
//     self.len()
//   }

//   fn map(&self, _: &MapOptions) -> Option<SourceMap> {
//     None
//   }
// }

// impl Source for str {
//   fn source(&self) -> Cow<str> {
//     Cow::Borrowed(self)
//   }

//   fn buffer(&self) -> Cow<[u8]> {
//     Cow::Borrowed(self.as_bytes())
//   }

//   fn size(&self) -> usize {
//     self.len()
//   }

//   fn map(&self, _: &MapOptions) -> Option<SourceMap> {
//     None
//   }
// }

// impl Source for Vec<u8> {
//   fn source(&self) -> Cow<str> {
//     String::from_utf8_lossy(self)
//   }

//   fn buffer(&self) -> Cow<[u8]> {
//     Cow::Borrowed(self)
//   }

//   fn size(&self) -> usize {
//     self.len()
//   }

//   fn map(&self, _: &MapOptions) -> Option<SourceMap> {
//     None
//   }
// }

// impl Source for [u8] {
//   fn source(&self) -> Cow<str> {
//     String::from_utf8_lossy(self)
//   }

//   fn buffer(&self) -> Cow<[u8]> {
//     Cow::Borrowed(self)
//   }

//   fn size(&self) -> usize {
//     self.len()
//   }

//   fn map(&self, _: &MapOptions) -> Option<SourceMap> {
//     None
//   }
// }

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RawSource {
  Buffer(Vec<u8>),
  Source(String),
}

impl RawSource {
  pub fn is_buffer(&self) -> bool {
    matches!(self, Self::Buffer(_))
  }
}

impl From<String> for RawSource {
  fn from(s: String) -> Self {
    Self::Source(s)
  }
}

impl From<Vec<u8>> for RawSource {
  fn from(s: Vec<u8>) -> Self {
    Self::Buffer(s)
  }
}

impl From<&str> for RawSource {
  fn from(s: &str) -> Self {
    Self::Source(s.to_owned())
  }
}

impl From<&[u8]> for RawSource {
  fn from(s: &[u8]) -> Self {
    Self::Buffer(s.to_owned())
  }
}

impl Source for RawSource {
  fn source(&self) -> Cow<str> {
    match self {
      RawSource::Buffer(i) => String::from_utf8_lossy(i),
      RawSource::Source(i) => Cow::Borrowed(i),
    }
  }

  fn buffer(&self) -> Cow<[u8]> {
    match self {
      RawSource::Buffer(i) => Cow::Borrowed(i),
      RawSource::Source(i) => Cow::Borrowed(i.as_bytes()),
    }
  }

  fn size(&self) -> usize {
    match self {
      RawSource::Buffer(i) => i.len(),
      RawSource::Source(i) => i.len(),
    }
  }

  fn map(&self, _: &MapOptions) -> Option<SourceMap> {
    None
  }
}

impl StreamChunks for RawSource {
  fn stream_chunks(
    &self,
    options: &MapOptions,
    on_chunk: OnChunk,
    _on_source: OnSource,
    _on_name: OnName,
  ) -> crate::helpers::GeneratedInfo {
    if options.final_source {
      get_generated_source_info(&self.source())
    } else {
      let mut line = 1;
      let mut last_line = None;
      let source = self.source();
      for l in split_into_lines(&source) {
        on_chunk(
          Some(l),
          Mapping {
            generated_line: line,
            generated_column: 0,
            original: None,
          },
        );
        line += 1;
        last_line = Some(l);
      }
      if let Some(last_line) = last_line && !last_line.ends_with('\n') {
        GeneratedInfo {
          generated_line: line,
          generated_column: last_line.len() as u32,
        }
      } else {
        GeneratedInfo {
          generated_line: line + 1,
          generated_column: 0,
        }
      }
    }
  }
}
