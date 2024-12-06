use std::{borrow::Cow, fmt::Display, ops::Range};

#[derive(Clone, Debug)]
pub struct Rope<'a> {
  data: Vec<(&'a str, usize)>,
}

impl<'a> Rope<'a> {
  /// Create a [Rope].
  pub const fn new() -> Self {
    Self { data: Vec::new() }
  }

  pub fn from_str(s: &'a str) -> Self {
    Self { data: vec![(s, 0)] }
  }

  pub fn append(&mut self, value: &'a str) {
    if !value.is_empty() {
      self.data.push((value, self.len()));
    }
  }

  pub fn extend(&mut self, value: impl IntoIterator<Item = &'a str>) {
    for value in value {
      self.append(value);
    }
  }

  pub fn chunks(&self) -> RopeChunks<'_, 'a> {
    self.iter()
  }

  pub fn iter(&self) -> RopeChunks<'_, 'a> {
    RopeChunks {
      data: &self.data,
      index: 0,
    }
  }

  pub fn starts_with(&self, value: &str) -> bool {
    if let Some((first, _)) = self.data.first() {
      first.starts_with(value)
    } else {
      false
    }
  }

  pub fn ends_with(&self, value: &str) -> bool {
    if let Some((last, _)) = self.data.last() {
      last.ends_with(value)
    } else {
      false
    }
  }

  pub fn is_empty(&self) -> bool {
    self.data.is_empty()
  }

  pub fn len(&self) -> usize {
    self
      .data
      .last()
      .map_or(0, |(text, start_pos)| start_pos + text.len())
  }

  /// # Panics
  ///
  /// Panics if the start of the range is greater than the end, or if the end is out of bounds (i.e. end > len_chars()).
  pub fn byte_slice(&self, range: Range<usize>) -> Rope<'a> {
    if range.end > self.len() {
      panic!("byte_slice end out of bounds");
    }

    if range.start >= self.len() {
      panic!("byte_slice start out of bounds");
    }

    if range.start > range.end {
      panic!("byte_slice start >= end");
    }

    // [start_chunk
    let start_chunk_index = self
      .data
      .binary_search_by(|(_, start_pos)| start_pos.cmp(&range.start))
      .unwrap_or_else(|insert_pos| {
        // insert pos could be 0
        insert_pos.saturating_sub(1)
      });

    // end_chunk)
    let end_chunk_index = self
      .data
      .binary_search_by(|(text, start_pos)| {
        let end_pos = start_pos + text.len(); // exclusive
        end_pos.cmp(&range.end)
      })
      .unwrap_or_else(|insert_pos| insert_pos);

    let mut rope = Rope::default();

    // [start_chunk, end_chunk]
    (start_chunk_index..=end_chunk_index).for_each(|i| {
      let (text, start_pos) = self.data[i];

      if start_chunk_index == i && end_chunk_index == i {
        let start = range.start - start_pos;
        let end = range.end - start_pos;
        rope.append(&text[start..end]);
      } else if start_chunk_index == i {
        let start = range.start - start_pos;
        rope.append(&text[start..]);
      } else if end_chunk_index == i {
        let end = range.end - start_pos;
        rope.append(&text[..end]);
      } else {
        rope.append(text);
      }
    });

    rope
  }

  pub fn to_bytes(&self) -> Vec<u8> {
    let mut bytes = Vec::new();
    for (chunk, _) in self.data.iter() {
      bytes.extend_from_slice(chunk.as_bytes());
    }
    bytes
  }
}

impl Default for Rope<'_> {
  fn default() -> Self {
    Self::new()
  }
}

impl<'a> Display for Rope<'a> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    for (chunk, _) in self.data.iter() {
      write!(f, "{}", chunk)?;
    }
    Ok(())
  }
}

impl PartialEq<str> for Rope<'_> {
  fn eq(&self, other: &str) -> bool {
    if self.len() != other.len() {
      return false;
    }

    let other = other.as_bytes();

    let mut idx = 0;
    for chunk in self.chunks() {
      let chunk = chunk.as_bytes();
      if chunk != &other[idx..(idx + chunk.len())] {
        return false;
      }
      idx += chunk.len();
    }

    true
  }
}

impl<'a> From<&'a str> for Rope<'a> {
  fn from(value: &'a str) -> Self {
    Rope {
      data: vec![(value, 0)],
    }
  }
}

impl<'a> From<&'a String> for Rope<'a> {
  fn from(value: &'a String) -> Self {
    Rope {
      data: vec![(&**value, 0)],
    }
  }
}

impl<'a> From<&'a Cow<'a, str>> for Rope<'a> {
  fn from(value: &'a Cow<'a, str>) -> Self {
    Rope {
      data: vec![(&**value, 0)],
    }
  }
}

pub struct RopeChunks<'a, 'b: 'a> {
  data: &'a Vec<(&'b str, usize)>,
  index: usize,
}

impl<'a, 'b: 'a> Iterator for RopeChunks<'a, 'b> {
  type Item = &'b str;

  fn next(&mut self) -> Option<Self::Item> {
    if self.index < self.data.len() {
      let (value, _) = self.data[self.index];
      self.index += 1;
      Some(value)
    } else {
      None
    }
  }
}

// impl std::io::Write for Rope<'_> {
//   fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
//     let s =
//       std::str::from_utf8(buf).map_err(|_| std::io::ErrorKind::InvalidData)?;
//     self.append(s);
//     Ok(buf.len())
//   }

//   fn flush(&mut self) -> std::io::Result<()> {
//     Ok(())
//   }
// }

#[cfg(test)]
mod tests {
  use crate::rope::Rope;

  #[test]
  fn append() {
    let mut r = Rope::new();
    r.append("a");
    r.append("b");
    assert_eq!(r.to_string(), "ab".to_string());
  }

  #[test]
  fn slice() {
    let mut a = Rope::new();
    a.append("abc");
    a.append("def");
    a.append("ghi");

    // same chunk start
    let rope = a.byte_slice(0..1);
    assert_eq!(rope.to_string(), "a".to_string());

    // same chunk end
    let rope = a.byte_slice(2..3);
    assert_eq!(rope.to_string(), "c".to_string());

    // cross chunks
    let rope = a.byte_slice(2..5);
    assert_eq!(rope.to_string(), "cde".to_string());

    // empty slice
    let rope = a.byte_slice(0..0);
    assert_eq!(rope.to_string(), "".to_string());
  }

  #[test]
  #[should_panic]
  fn slice_panics_range_start_out_of_bound() {
    let mut a = Rope::new();
    a.append("abc");
    a.byte_slice(3..3);
  }

  #[test]
  #[should_panic]
  fn slice_panics_range_start_greater_than_end() {
    let mut a = Rope::new();
    a.append("abc");
    a.byte_slice(1..0);
  }

  #[test]
  #[should_panic]
  fn slice_panics_range_end_out_of_bound() {
    let mut a = Rope::new();
    a.append("abc");
    a.byte_slice(0..4);
  }

  #[test]
  fn eq() {
    let mut a = Rope::new();
    a.append("abc");
    a.append("def");
    a.append("ghi");

    assert_eq!(&a, "abcdefghi");
  }

  #[test]
  fn from() {
    let _ = Rope::from_str("abc");
    let _ = Rope::from("abc");
  }
}
