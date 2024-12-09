use core::panic;
use std::{
  borrow::Cow,
  cell::RefCell,
  collections::VecDeque,
  fmt::Display,
  hash::Hash,
  ops::{Bound, Index, Range, RangeBounds},
};

use crate::Error;

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

  pub fn chunks(&self) -> Chunks<'_, 'a> {
    self.iter()
  }

  pub fn iter(&self) -> Chunks<'_, 'a> {
    Chunks {
      data: &self.data,
      index: 0,
    }
  }

  /// # Panics
  ///
  /// Panics if the index is out of bounds.
  pub fn byte(&self, byte_index: usize) -> u8 {
    self.get_byte(byte_index).expect("byte out of bounds")
  }

  pub fn get_byte(&self, byte_index: usize) -> Option<u8> {
    if byte_index >= self.len() {
      return None;
    }
    let chunk_index = self
      .data
      .binary_search_by(|(_, start_pos)| start_pos.cmp(&byte_index))
      .unwrap_or_else(|index| index.saturating_sub(1));
    let (s, start_pos) = &self.data.get(chunk_index)?;
    let pos = byte_index - start_pos;
    Some(s.as_bytes()[pos])
  }

  pub fn char_indices(&self) -> CharIndices<'_, 'a> {
    CharIndices {
      chunks: &self.data,
      char_indices: Default::default(),
      chunk_index: 0,
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
    self.data.iter().all(|(s, _)| s.is_empty())
  }

  pub fn len(&self) -> usize {
    self
      .data
      .last()
      .map_or(0, |(text, start_pos)| start_pos + text.len())
  }

  /// # Panics
  ///
  /// Panics if the start of the range is greater than the end, or if the end is out of bounds.
  pub fn byte_slice<R>(&self, range: R) -> Rope<'a>
  where
    R: RangeBounds<usize>,
  {
    self.get_byte_slice_impl(range).unwrap_or_else(|e| {
      panic!("byte_slice: {}", e);
    })
  }

  pub fn get_byte_slice<R>(&self, range: R) -> Option<Rope<'a>>
  where
    R: RangeBounds<usize>,
  {
    self.get_byte_slice_impl(range).ok()
  }

  pub(crate) fn get_byte_slice_impl<R>(
    &self,
    range: R,
  ) -> Result<Rope<'a>, Error>
  where
    R: RangeBounds<usize>,
  {
    let start_range = start_bound_to_range_start(range.start_bound());
    let end_range = end_bound_to_range_end(range.end_bound());

    match (start_range, end_range) {
      (Some(start), Some(end)) => {
        if start > end {
          return Err(Error::Rope("start >= end"));
        } else if end > self.len() {
          return Err(Error::Rope("end out of bounds"));
        }
      }
      (None, Some(end)) => {
        if end > self.len() {
          return Err(Error::Rope("end out of bounds"));
        }
      }
      (Some(start), None) => {
        if start > self.len() {
          return Err(Error::Rope("start out of bounds"));
        }
      }
      _ => {}
    }

    let start_range = start_range.unwrap_or(0);
    let end_range = end_range.unwrap_or_else(|| self.len());

    // [start_chunk
    let start_chunk_index = self
      .data
      .binary_search_by(|(_, start_pos)| start_pos.cmp(&start_range))
      .unwrap_or_else(|insert_pos| insert_pos.saturating_sub(1));

    // end_chunk)
    let end_chunk_index = self
      .data
      .binary_search_by(|(text, start_pos)| {
        let end_pos = start_pos + text.len(); // exclusive
        end_pos.cmp(&end_range)
      })
      .unwrap_or_else(|insert_pos| insert_pos);

    let mut rope = Rope::default();

    // [start_chunk, end_chunk]
    (start_chunk_index..=end_chunk_index).try_for_each(|i| {
      let (text, start_pos) = self.data[i];

      if start_chunk_index == i && end_chunk_index == i {
        let start = start_range - start_pos;
        let end = end_range - start_pos;
        if text.is_char_boundary(start) && text.is_char_boundary(end) {
          rope.append(&text[start..end]);
        } else {
          return Err(Error::Rope("invalid char boundary"));
        }
      } else if start_chunk_index == i {
        let start = start_range - start_pos;
        if text.is_char_boundary(start) {
          rope.append(&text[start..]);
        } else {
          return Err(Error::Rope("invalid char boundary"));
        }
      } else if end_chunk_index == i {
        let end = end_range - start_pos;
        if text.is_char_boundary(end) {
          rope.append(&text[..end]);
        } else {
          return Err(Error::Rope("invalid char boundary"));
        }
      } else {
        rope.append(text);
      }

      Ok(())
    })?;

    Ok(rope)
  }

  pub fn to_bytes(&self) -> Vec<u8> {
    let mut bytes = Vec::new();
    for (chunk, _) in self.data.iter() {
      bytes.extend_from_slice(chunk.as_bytes());
    }
    bytes
  }
}

impl Hash for Rope<'_> {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    for (s, _) in &self.data {
      s.hash(state);
    }
  }
}

pub struct CharIndices<'a, 'b> {
  chunks: &'a [(&'b str, usize)],
  char_indices: VecDeque<(usize, char)>,
  chunk_index: usize,
}

impl<'a, 'b> Iterator for CharIndices<'a, 'b> {
  type Item = (usize, char);

  fn next(&mut self) -> Option<Self::Item> {
    if let Some(item) = self.char_indices.pop_front() {
      return Some(item);
    }

    if self.chunk_index >= self.chunks.len() {
      return None;
    }

    if self.char_indices.is_empty() {
      let (chunk, start_pos) = self.chunks[self.chunk_index];
      self
        .char_indices
        .extend(chunk.char_indices().map(|(i, c)| (start_pos + i, c)));
      self.chunk_index += 1;
    }

    self.char_indices.pop_front()
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

impl PartialEq<Rope<'_>> for Rope<'_> {
  fn eq(&self, other: &Rope<'_>) -> bool {
    if self.len() != other.len() {
      return false;
    }

    if self.len() == 0 {
      return true;
    }

    let chunks = &self.data;
    let other_chunks = &other.data;

    let mut cur = 0;
    let other_chunk_index = RefCell::new(0);
    let mut other_chunk_byte_index = 0;
    let other_chunk = || other_chunks[*other_chunk_index.borrow()].0.as_bytes();
    for (chunk, start_pos) in chunks {
      let chunk = chunk.as_bytes();
      while (cur - start_pos) < chunk.len() {
        if other_chunk_byte_index >= other_chunk().len() {
          other_chunk_byte_index = 0;
          *other_chunk_index.borrow_mut() += 1;
        }
        if chunk[cur - start_pos] == other_chunk()[other_chunk_byte_index] {
          cur += 1;
          other_chunk_byte_index += 1;
        } else {
          return false;
        }
      }
    }

    true
  }
}

impl PartialEq<str> for Rope<'_> {
  fn eq(&self, other: &str) -> bool {
    if self.len() != other.len() {
      return false;
    }

    let other = other.as_bytes();

    let mut idx = 0;
    for (chunk, _) in &self.data {
      let chunk = chunk.as_bytes();
      if chunk != &other[idx..(idx + chunk.len())] {
        return false;
      }
      idx += chunk.len();
    }

    true
  }
}

impl PartialEq<&str> for Rope<'_> {
  fn eq(&self, other: &&str) -> bool {
    if self.len() != other.len() {
      return false;
    }

    let other = other.as_bytes();

    let mut idx = 0;
    for (chunk, _) in &self.data {
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

pub struct Chunks<'a, 'b: 'a> {
  data: &'a Vec<(&'b str, usize)>,
  index: usize,
}

impl<'a, 'b: 'a> Index<usize> for Chunks<'a, 'b> {
  type Output = (&'b str, usize);

  fn index(&self, index: usize) -> &Self::Output {
    &self.data[index]
  }
}

impl<'a, 'b: 'a> Iterator for Chunks<'a, 'b> {
  type Item = (&'b str, usize);

  fn next(&mut self) -> Option<Self::Item> {
    if self.index < self.data.len() {
      let (value, pos) = self.data[self.index];
      self.index += 1;
      Some((value, pos))
    } else {
      None
    }
  }
}

fn start_bound_to_range_start(start: Bound<&usize>) -> Option<usize> {
  match start {
    Bound::Included(&start) => Some(start),
    Bound::Excluded(&start) => Some(start + 1),
    Bound::Unbounded => None,
  }
}

fn end_bound_to_range_end(end: Bound<&usize>) -> Option<usize> {
  match end {
    Bound::Included(&end) => Some(end + 1),
    Bound::Excluded(&end) => Some(end),
    Bound::Unbounded => None,
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

    // slice with len
    let rope = Rope::from_str("abc");
    let rope = rope.byte_slice(3..3);
    assert_eq!(rope.to_string(), "".to_string())
  }

  #[test]
  #[should_panic]
  fn slice_panics_range_start_out_of_bounds() {
    let mut a = Rope::new();
    a.append("abc");
    a.byte_slice(3..4);
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
  fn slice_panics_range_end_out_of_bounds() {
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
    assert_eq!(a, "abcdefghi");

    let mut b = Rope::new();
    b.append("abcde");
    b.append("fghi");

    assert_eq!(a, b);
  }

  #[test]
  fn from() {
    let _ = Rope::from_str("abc");
    let _ = Rope::from("abc");
  }

  #[test]
  fn byte() {
    let mut a = Rope::from_str("abc");
    assert_eq!(a.byte(0), b'a');
    a.append("d");
    assert_eq!(a.byte(3), b'd');
  }

  #[test]
  fn char_indices() {
    let mut a = Rope::new();
    a.append("abc");
    a.append("def");

    let a = a.char_indices().collect::<Vec<_>>();
    let b = "abcdef".char_indices().collect::<Vec<_>>();

    assert_eq!(a, b);
  }
}
