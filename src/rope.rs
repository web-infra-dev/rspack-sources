#![allow(unsafe_code)]

use std::{
  borrow::Cow,
  cell::RefCell,
  collections::VecDeque,
  hash::Hash,
  ops::{Bound, RangeBounds},
  rc::Rc,
};

use crate::Error;

#[derive(Clone, Debug)]
pub(crate) enum Repr<'a> {
  Simple(&'a str),
  Complex(Rc<Vec<(&'a str, usize)>>),
}

/// A rope data structure.
#[derive(Clone, Debug)]
pub struct Rope<'a> {
  repr: Repr<'a>,
}

impl<'a> Rope<'a> {
  /// Creates a new empty rope.
  pub const fn new() -> Self {
    Self {
      repr: Repr::Simple(""),
    }
  }

  /// Adds a string slice to the end of the rope.
  ///
  /// Converts from simple to complex representation on first add.
  /// Empty strings are ignored.
  pub fn add(&mut self, value: &'a str) {
    if value.is_empty() {
      return;
    }

    match &mut self.repr {
      Repr::Simple(s) => {
        let vec = Vec::from_iter([(*s, 0), (value, s.len())]);
        self.repr = Repr::Complex(Rc::new(vec));
      }
      Repr::Complex(data) => {
        let len = data
          .last()
          .map_or(0, |(chunk, start_pos)| *start_pos + chunk.len());
        Rc::make_mut(data).push((value, len));
      }
    }
  }

  /// Appends another rope to this rope.
  ///
  /// Handles all combinations of simple and complex representations efficiently.
  pub fn append(&mut self, value: Rope<'a>) {
    match (&mut self.repr, value.repr) {
      (Repr::Simple(s), Repr::Simple(other)) => {
        let raw = Vec::from_iter([(*s, 0), (other, s.len())]);
        self.repr = Repr::Complex(Rc::new(raw));
      }
      (Repr::Complex(s), Repr::Complex(other)) => {
        if !other.is_empty() {
          let mut len = s
            .last()
            .map_or(0, |(chunk, start_pos)| *start_pos + chunk.len());

          let cur = Rc::make_mut(s);
          cur.reserve_exact(other.len());

          for &(chunk, _) in other.iter() {
            cur.push((chunk, len));
            len += chunk.len();
          }
        }
      }
      (Repr::Complex(s), Repr::Simple(other)) => {
        if !other.is_empty() {
          let len = s
            .last()
            .map_or(0, |(chunk, start_pos)| *start_pos + chunk.len());
          Rc::make_mut(s).push((other, len));
        }
      }
      (Repr::Simple(s), Repr::Complex(other)) => {
        let mut raw = Vec::with_capacity(other.len() + 1);
        raw.push((*s, 0));
        let mut len = s.len();
        for &(chunk, _) in other.iter() {
          raw.push((chunk, len));
          len += chunk.len();
        }
        self.repr = Repr::Complex(Rc::new(raw));
      }
    }
  }

  /// Gets the byte at the given index.
  ///
  /// # Panics
  /// When index is out of bounds.
  pub fn byte(&self, byte_index: usize) -> u8 {
    self.get_byte(byte_index).expect("byte out of bounds")
  }

  /// Non-panicking version of [Rope::byte].
  ///
  /// Gets the byte at the given index, returning None if out of bounds.
  pub fn get_byte(&self, byte_index: usize) -> Option<u8> {
    if byte_index >= self.len() {
      return None;
    }
    match &self.repr {
      Repr::Simple(s) => Some(s.as_bytes()[byte_index]),
      Repr::Complex(data) => {
        let chunk_index = data
          .binary_search_by(|(_, start_pos)| start_pos.cmp(&byte_index))
          .unwrap_or_else(|index| index.saturating_sub(1));
        let (s, start_pos) = &data.get(chunk_index)?;
        let pos = byte_index - start_pos;
        Some(s.as_bytes()[pos])
      }
    }
  }

  /// Returns an iterator over the characters and their byte positions.
  pub fn char_indices(&self) -> CharIndices<'_, 'a> {
    match &self.repr {
      Repr::Simple(s) => CharIndices {
        repr: CharIndicesRepr::Simple {
          iter: s.char_indices(),
        },
      },
      Repr::Complex(data) => CharIndices {
        repr: CharIndicesRepr::Complex {
          chunks: data,
          char_indices: VecDeque::new(),
          chunk_index: 0,
        },
      },
    }
  }

  /// Returns whether the rope starts with the given string.
  #[inline]
  pub fn starts_with(&self, value: &str) -> bool {
    match &self.repr {
      Repr::Simple(s) => s.starts_with(value),
      Repr::Complex(data) => {
        if let Some((first, _)) = data.first() {
          first.starts_with(value)
        } else {
          false
        }
      }
    }
  }

  /// Returns whether the rope ends with the given string.
  #[inline]
  pub fn ends_with(&self, value: &str) -> bool {
    match &self.repr {
      Repr::Simple(s) => s.ends_with(value),
      Repr::Complex(data) => {
        if let Some((last, _)) = data.last() {
          last.ends_with(value)
        } else {
          false
        }
      }
    }
  }

  /// Returns whether the rope is empty.
  #[inline]
  pub fn is_empty(&self) -> bool {
    match &self.repr {
      Repr::Simple(s) => s.is_empty(),
      Repr::Complex(data) => data.iter().all(|(s, _)| s.is_empty()),
    }
  }

  /// Returns the length of the rope in bytes.
  #[inline]
  pub fn len(&self) -> usize {
    match &self.repr {
      Repr::Simple(s) => s.len(),
      Repr::Complex(data) => data
        .last()
        .map_or(0, |(chunk, start_pos)| start_pos + chunk.len()),
    }
  }

  /// Returns a slice of the rope in the given byte range.
  ///
  /// # Panics
  /// - When start > end
  /// - When end is out of bounds
  /// - When indices are not on char boundaries
  pub fn byte_slice<R>(&self, range: R) -> Rope<'a>
  where
    R: RangeBounds<usize>,
  {
    self.get_byte_slice_impl(range).unwrap_or_else(|e| {
      panic!("byte_slice: {}", e);
    })
  }

  /// Non-panicking version of [Rope::byte_slice].
  pub fn get_byte_slice<R>(&self, range: R) -> Option<Rope<'a>>
  where
    R: RangeBounds<usize>,
  {
    self.get_byte_slice_impl(range).ok()
  }

  /// Implementation for byte_slice operations.
  #[inline]
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

    match &self.repr {
      Repr::Simple(s) => s
        .get(start_range..end_range)
        .map(Rope::from)
        .ok_or(Error::Rope("invalid char boundary")),
      Repr::Complex(data) => {
        // [start_chunk
        let start_chunk_index = data
          .binary_search_by(|(_, start_pos)| start_pos.cmp(&start_range))
          .unwrap_or_else(|insert_pos| insert_pos.saturating_sub(1));

        // end_chunk)
        let end_chunk_index = data
          .binary_search_by(|(chunk, start_pos)| {
            let end_pos = start_pos + chunk.len(); // exclusive
            end_pos.cmp(&end_range)
          })
          .unwrap_or_else(|insert_pos| insert_pos);

        // same chunk
        if start_chunk_index == end_chunk_index {
          // SAFETY: start_chunk_index guarantees valid range
          let (chunk, start_pos) =
            unsafe { data.get_unchecked(start_chunk_index) };
          let start = start_range - start_pos;
          let end = end_range - start_pos;
          return chunk
            .get(start..end)
            .map(Rope::from)
            .ok_or(Error::Rope("invalid char boundary"));
        }

        if end_chunk_index < start_chunk_index {
          return Ok(Rope::new());
        }

        let mut raw =
          Vec::with_capacity(end_chunk_index - start_chunk_index + 1);
        let mut len = 0;

        // different chunk
        // [start_chunk, end_chunk]
        (start_chunk_index..end_chunk_index + 1).try_for_each(|i| {
          // SAFETY: [start_chunk_index, end_chunk_index] guarantees valid range
          let (chunk, start_pos) = unsafe { data.get_unchecked(i) };

          if start_chunk_index == i {
            let start = start_range - start_pos;
            if let Some(chunk) = chunk.get(start..) {
              raw.push((chunk, len));
              len += chunk.len();
            } else {
              return Err(Error::Rope("invalid char boundary"));
            }
          } else if end_chunk_index == i {
            let end = end_range - start_pos;
            if let Some(chunk) = chunk.get(..end) {
              raw.push((chunk, len));
              len += chunk.len();
            } else {
              return Err(Error::Rope("invalid char boundary"));
            }
          } else {
            raw.push((chunk, len));
            len += chunk.len();
          }

          Ok(())
        })?;

        Ok(Rope {
          repr: Repr::Complex(Rc::new(raw)),
        })
      }
    }
  }

  /// Range-unchecked version of [Rope::byte_slice].
  ///
  /// # Safety
  ///
  /// This is not safe, due to the following invariants that must be upheld:
  ///
  /// - Range must be within bounds.
  /// - Range start must be less than or equal to the end.
  /// - Both range start and end must be on char boundaries.
  pub unsafe fn byte_slice_unchecked<R>(&self, range: R) -> Rope<'a>
  where
    R: RangeBounds<usize>,
  {
    let start_range = start_bound_to_range_start(range.start_bound());
    let end_range = end_bound_to_range_end(range.end_bound());

    let start_range = start_range.unwrap_or(0);
    let end_range = end_range.unwrap_or_else(|| self.len());

    match &self.repr {
      Repr::Simple(s) => {
        // SAFETY: invariant guarantees valid range
        Rope::from(unsafe { s.get_unchecked(start_range..end_range) })
      }
      Repr::Complex(data) => {
        // [start_chunk
        let start_chunk_index = data
          .binary_search_by(|(_, start_pos)| start_pos.cmp(&start_range))
          .unwrap_or_else(|insert_pos| insert_pos.saturating_sub(1));

        // end_chunk)
        let end_chunk_index = data
          .binary_search_by(|(chunk, start_pos)| {
            let end_pos = start_pos + chunk.len(); // exclusive
            end_pos.cmp(&end_range)
          })
          .unwrap_or_else(|insert_pos| insert_pos);

        // same chunk
        if start_chunk_index == end_chunk_index {
          // SAFETY: start_chunk_index guarantees valid range
          let (chunk, start_pos) =
            unsafe { data.get_unchecked(start_chunk_index) };
          let start = start_range - start_pos;
          let end = end_range - start_pos;
          // SAFETY: invariant guarantees valid range
          return Rope::from(unsafe { chunk.get_unchecked(start..end) });
        }

        if end_chunk_index < start_chunk_index {
          return Rope::new();
        }

        let mut raw =
          Vec::with_capacity(end_chunk_index - start_chunk_index + 1);
        let mut len = 0;

        // different chunk
        // [start_chunk, end_chunk]
        (start_chunk_index..end_chunk_index + 1).for_each(|i| {
          // SAFETY: [start_chunk_index, end_chunk_index] guarantees valid range
          let (chunk, start_pos) = unsafe { data.get_unchecked(i) };

          if start_chunk_index == i {
            let start = start_range - start_pos;
            // SAFETY: invariant guarantees valid range
            let chunk = unsafe { chunk.get_unchecked(start..) };
            raw.push((chunk, len));
            len += chunk.len();
          } else if end_chunk_index == i {
            let end = end_range - start_pos;
            // SAFETY: invariant guarantees valid range
            let chunk = unsafe { chunk.get_unchecked(..end) };
            raw.push((chunk, len));
            len += chunk.len();
          } else {
            raw.push((chunk, len));
            len += chunk.len();
          }
        });

        Rope {
          repr: Repr::Complex(Rc::new(raw)),
        }
      }
    }
  }

  /// Returns an iterator over the lines of the rope.
  pub fn lines(&self) -> Lines<'_, 'a> {
    self.lines_impl(true)
  }

  /// Returns an iterator over the lines of the rope.
  ///
  /// If `trailing_line_break_as_newline` is true, the end of the rope with ('\n') is treated as an empty newline
  pub(crate) fn lines_impl(
    &self,
    trailing_line_break_as_newline: bool,
  ) -> Lines<'_, 'a> {
    Lines {
      iter: match &self.repr {
        Repr::Simple(s) => LinesEnum::Simple(s),
        Repr::Complex(data) => LinesEnum::Complex {
          iter: data,
          in_chunk_byte_idx: 0,
          chunk_idx: 0,
        },
      },
      byte_idx: 0,
      ended: false,
      total_bytes: self.len(),
      trailing_line_break_as_newline,
    }
  }

  /// Converts the rope to bytes.
  ///
  /// Returns borrowed bytes for simple ropes and owned bytes for complex ropes.
  pub fn to_bytes(&self) -> Cow<'a, [u8]> {
    match &self.repr {
      Repr::Simple(s) => Cow::Borrowed(s.as_bytes()),
      Repr::Complex(data) => {
        let mut bytes = vec![];
        for (chunk, _) in data.iter() {
          bytes.extend_from_slice(chunk.as_bytes());
        }
        Cow::Owned(bytes)
      }
    }
  }

  /// Returns the underlying &str if this is a simple rope.
  pub fn get_simple(&self) -> Option<&'a str> {
    match &self.repr {
      Repr::Simple(s) => Some(s),
      _ => None,
    }
  }
}

impl Hash for Rope<'_> {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    match &self.repr {
      Repr::Simple(s) => s.hash(state),
      Repr::Complex(data) => {
        for (s, _) in data.iter() {
          s.hash(state);
        }
      }
    }
  }
}

enum LinesEnum<'a, 'b> {
  Simple(&'b str),
  Complex {
    iter: &'a Vec<(&'b str, usize)>,
    in_chunk_byte_idx: usize,
    chunk_idx: usize,
  },
}

pub struct Lines<'a, 'b> {
  iter: LinesEnum<'a, 'b>,
  byte_idx: usize,
  ended: bool,
  total_bytes: usize,

  /// Whether to treat the end of the rope with ('\n') as an empty newline.
  trailing_line_break_as_newline: bool,
}

impl<'a> Iterator for Lines<'_, 'a> {
  type Item = Rope<'a>;

  fn next(&mut self) -> Option<Self::Item> {
    match *self {
      Lines {
        iter: LinesEnum::Simple(s),
        ref mut byte_idx,
        ref mut ended,
        ref total_bytes,
        trailing_line_break_as_newline,
        ..
      } => {
        if *ended {
          return None;
        } else if byte_idx == total_bytes {
          if trailing_line_break_as_newline {
            *ended = true;
            return Some(Rope::from(""));
          }
          return None;
        } else if let Some(idx) =
          memchr::memchr(b'\n', &s.as_bytes()[*byte_idx..])
        {
          let end = *byte_idx + idx + 1;
          let rope = Rope::from(&s[*byte_idx..end]);
          *byte_idx = end;
          return Some(rope);
        }
        *ended = true;
        Some(Rope::from(&s[*byte_idx..]))
      }
      Lines {
        iter:
          LinesEnum::Complex {
            iter: chunks,
            ref mut in_chunk_byte_idx,
            ref mut chunk_idx,
          },
        ref mut byte_idx,
        ref mut ended,
        ref total_bytes,
        trailing_line_break_as_newline,
      } => {
        if *ended {
          return None;
        } else if byte_idx == total_bytes {
          if trailing_line_break_as_newline {
            *ended = true;
            return Some(Rope::from(""));
          }
          return None;
        }

        debug_assert!(*chunk_idx < chunks.len());
        let &(chunk, _) = &chunks[*chunk_idx];

        // Always try to find a newline in the current chunk,
        // if the current chunk contains a newline, return this line.
        if let Some(idx) =
          memchr::memchr(b'\n', &chunk.as_bytes()[*in_chunk_byte_idx..])
        {
          let end = *in_chunk_byte_idx + idx + 1;
          let rope = Rope::from(&chunk[*in_chunk_byte_idx..end]);
          *in_chunk_byte_idx = end;
          *byte_idx += *in_chunk_byte_idx;
          Some(rope)
        } else {
          // Check if the current chunk has left over bytes.
          // If it is the last chunk, return the remaining bytes.
          // This is the end of the rope.
          if *chunk_idx == chunks.len() - 1 {
            // Rope is not ended with a newline.
            // Explicitly set the ended flag to true to bail out.
            *ended = true;
            *byte_idx += chunk.len() - *in_chunk_byte_idx;
            return Some(Rope::from(&chunk[*in_chunk_byte_idx..]));
          }

          // If the current chunk has running out of bytes, move to the next chunk.
          if *in_chunk_byte_idx == chunk.len() {
            *chunk_idx += 1;
            *in_chunk_byte_idx = 0;
            return self.next();
          }

          // If it is not the last chunk, the line spans multiple chunks.
          // As such, we need to find the next newline in the next few chunks.
          let start_chunk_idx = *chunk_idx;
          let start_in_chunk_byte_idx = *in_chunk_byte_idx;

          let end_info = loop {
            if *chunk_idx == chunks.len() {
              break None;
            }
            let &(chunk, _) = &chunks[*chunk_idx];
            if let Some(idx) =
              memchr::memchr(b'\n', &chunk.as_bytes()[*in_chunk_byte_idx..])
            {
              *in_chunk_byte_idx += idx + 1;
              *byte_idx += *in_chunk_byte_idx;
              break Some((*chunk_idx, *in_chunk_byte_idx));
            } else {
              *in_chunk_byte_idx = 0;
              *byte_idx += chunk.len();
              *chunk_idx += 1;
            }
          };

          // If we found a newline in the next few chunks, return the line.
          if let Some((end_chunk_idx, end_in_chunk_byte_idx)) = end_info {
            let mut raw =
              Vec::with_capacity(end_chunk_idx - start_chunk_idx + 1);
            let mut len = 0;
            (start_chunk_idx..end_chunk_idx + 1).for_each(|i| {
              let &(chunk, _) = &chunks[i];
              if start_chunk_idx == i {
                let start = start_in_chunk_byte_idx;
                raw.push((&chunk[start..], len));
                len += chunk.len() - start;
              } else if end_chunk_idx == i {
                let end = end_in_chunk_byte_idx;
                raw.push((&chunk[..end], len));
                len += end;
              } else {
                raw.push((chunk, len));
                len += chunk.len();
              }
            });
            // Advance the byte index to the end of the line.
            *byte_idx += len;
            Some(Rope {
              repr: Repr::Complex(Rc::new(raw)),
            })
          } else {
            // If we did not find a newline in the next few chunks,
            // return the remaining bytes.

            let mut raw = Vec::with_capacity(chunks.len() - start_chunk_idx);
            let mut len = 0;
            (start_chunk_idx..chunks.len()).for_each(|i| {
              let &(chunk, _) = &chunks[i];
              if start_chunk_idx == i {
                let start = start_in_chunk_byte_idx;
                raw.push((&chunk[start..], len));
                len += chunk.len() - start;
              } else {
                raw.push((chunk, len));
                len += chunk.len();
              }
            });
            // Advance the byte index to the end of the rope.
            *byte_idx += len;
            Some(Rope {
              repr: Repr::Complex(Rc::new(raw)),
            })
          }
        }
      }
    }
  }
}

enum CharIndicesRepr<'a, 'b> {
  Simple {
    iter: std::str::CharIndices<'b>,
  },
  Complex {
    chunks: &'a [(&'b str, usize)],
    char_indices: VecDeque<(usize, char)>,
    chunk_index: usize,
  },
}

pub struct CharIndices<'a, 'b> {
  repr: CharIndicesRepr<'a, 'b>,
}

impl Iterator for CharIndices<'_, '_> {
  type Item = (usize, char);

  fn next(&mut self) -> Option<Self::Item> {
    match &mut self.repr {
      CharIndicesRepr::Simple { iter } => iter.next(),
      CharIndicesRepr::Complex {
        chunks,
        char_indices,
        chunk_index,
      } => {
        if let Some(item) = char_indices.pop_front() {
          return Some(item);
        }

        if *chunk_index >= chunks.len() {
          return None;
        }

        // skip empty chunks
        while *chunk_index < chunks.len() && chunks[*chunk_index].0.is_empty() {
          *chunk_index += 1;
        }

        let (chunk, start_pos) = chunks[*chunk_index];

        char_indices
          .extend(chunk.char_indices().map(|(i, c)| (start_pos + i, c)));
        *chunk_index += 1;
        char_indices.pop_front()
      }
    }
  }
}

impl Default for Rope<'_> {
  fn default() -> Self {
    Self::new()
  }
}

// Implement `ToString` than `Display` to manually allocate the string with capacity.
// This is faster than using `Display` and `write!` for large ropes.
#[allow(clippy::to_string_trait_impl)]
impl ToString for Rope<'_> {
  fn to_string(&self) -> String {
    match &self.repr {
      Repr::Simple(s) => s.to_string(),
      Repr::Complex(data) => {
        let mut s = String::with_capacity(self.len());
        for (chunk, _) in data.iter() {
          s.push_str(chunk);
        }
        s
      }
    }
  }
}

impl PartialEq<Rope<'_>> for Rope<'_> {
  fn eq(&self, other: &Rope<'_>) -> bool {
    if self.len() != other.len() {
      return false;
    }

    let chunks = match &self.repr {
      Repr::Simple(s) => &[(*s, 0)][..],
      Repr::Complex(data) => &data[..],
    };
    let other_chunks = match &other.repr {
      Repr::Simple(s) => &[(*s, 0)][..],
      Repr::Complex(data) => &data[..],
    };

    let mut cur = 0;
    let other_chunk_index = RefCell::new(0);
    let mut other_chunk_byte_index = 0;
    let other_chunk = || other_chunks[*other_chunk_index.borrow()].0.as_bytes();
    for (chunk, start_pos) in chunks.iter() {
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

    match &self.repr {
      Repr::Simple(s) => {
        if s.as_bytes() != other {
          return false;
        }
      }
      Repr::Complex(data) => {
        let mut idx = 0;
        for (chunk, _) in data.iter() {
          let chunk = chunk.as_bytes();
          if chunk != &other[idx..(idx + chunk.len())] {
            return false;
          }
          idx += chunk.len();
        }
      }
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

    match &self.repr {
      Repr::Simple(s) => {
        if s.as_bytes() != other {
          return false;
        }
      }
      Repr::Complex(data) => {
        let mut idx = 0;
        for (chunk, _) in data.iter() {
          let chunk = chunk.as_bytes();
          if chunk != &other[idx..(idx + chunk.len())] {
            return false;
          }
          idx += chunk.len();
        }
      }
    }

    true
  }
}

impl<'a> From<&'a str> for Rope<'a> {
  fn from(value: &'a str) -> Self {
    Rope {
      repr: Repr::Simple(value),
    }
  }
}

impl<'a> From<&'a String> for Rope<'a> {
  fn from(value: &'a String) -> Self {
    Rope {
      repr: Repr::Simple(value),
    }
  }
}

impl<'a> From<&'a Cow<'a, str>> for Rope<'a> {
  fn from(value: &'a Cow<'a, str>) -> Self {
    Rope {
      repr: Repr::Simple(value),
    }
  }
}

impl<'a> FromIterator<&'a str> for Rope<'a> {
  fn from_iter<T: IntoIterator<Item = &'a str>>(iter: T) -> Self {
    let mut len = 0;
    let raw = iter
      .into_iter()
      .map(|chunk| {
        let cur = (chunk, len);
        len += chunk.len();
        cur
      })
      .collect::<Vec<_>>();

    Self {
      repr: Repr::Complex(Rc::new(raw)),
    }
  }
}

#[inline(always)]
fn start_bound_to_range_start(start: Bound<&usize>) -> Option<usize> {
  match start {
    Bound::Included(&start) => Some(start),
    Bound::Excluded(&start) => Some(start + 1),
    Bound::Unbounded => None,
  }
}

#[inline(always)]
fn end_bound_to_range_end(end: Bound<&usize>) -> Option<usize> {
  match end {
    Bound::Included(&end) => Some(end + 1),
    Bound::Excluded(&end) => Some(end),
    Bound::Unbounded => None,
  }
}

#[cfg(test)]
mod tests {
  use std::rc::Rc;

  use crate::rope::{Repr, Rope};

  impl<'a> PartialEq for Repr<'a> {
    fn eq(&self, other: &Self) -> bool {
      match (self, other) {
        (Repr::Simple(a), Repr::Simple(b)) => a == b,
        (Repr::Complex(a), Repr::Complex(b)) => a == b,
        _ => false,
      }
    }
  }

  impl<'a> Eq for Repr<'a> {}

  #[test]
  fn add() {
    let mut simple = Rope::from("abc");
    assert_eq!(simple.repr, Repr::Simple("abc"));
    assert_eq!(simple.len(), 3);

    simple.add("def");
    assert_eq!(simple, "abcdef");
    assert_eq!(
      simple.repr,
      Repr::Complex(Rc::new(Vec::from_iter([("abc", 0), ("def", 3)])))
    );
    assert_eq!(simple.len(), 6);

    simple.add("ghi");
    assert_eq!(simple, "abcdefghi");
    assert_eq!(
      simple.repr,
      Repr::Complex(Rc::new(Vec::from_iter([
        ("abc", 0),
        ("def", 3),
        ("ghi", 6),
      ])))
    );
    assert_eq!(simple.len(), 9);
  }

  #[test]
  fn append() {
    let simple1 = Rope::from("abc");
    let simple2 = Rope::from("def");

    let complex1 = Rope::from_iter(["1", "2", "3"]);
    let complex2 = Rope::from_iter(["4", "5", "6"]);

    // simple - simple
    let mut append1 = simple1.clone();
    append1.append(simple2.clone());
    assert_eq!(append1, "abcdef");
    assert_eq!(
      append1.repr,
      Repr::Complex(Rc::new(Vec::from_iter([("abc", 0), ("def", 3),])))
    );

    // simple - complex
    let mut append2 = simple1.clone();
    append2.append(complex1.clone());
    assert_eq!(append2, "abc123");
    assert_eq!(
      append2.repr,
      Repr::Complex(Rc::new(Vec::from_iter([
        ("abc", 0),
        ("1", 3),
        ("2", 4),
        ("3", 5),
      ])))
    );

    // complex - simple
    let mut append3 = complex1.clone();
    append3.append(simple1.clone());
    assert_eq!(append3, "123abc");
    assert_eq!(
      append3.repr,
      Repr::Complex(Rc::new(Vec::from_iter([
        ("1", 0),
        ("2", 1),
        ("3", 2),
        ("abc", 3),
      ])))
    );

    // complex - complex
    let mut append4 = complex1.clone();
    append4.append(complex2.clone());
    assert_eq!(append4, "123456");
    assert_eq!(
      append4.repr,
      Repr::Complex(Rc::new(Vec::from_iter([
        ("1", 0),
        ("2", 1),
        ("3", 2),
        ("4", 3),
        ("5", 4),
        ("6", 5),
      ])))
    );
  }

  #[test]
  fn slice() {
    let mut a = Rope::new();
    a.add("abc");
    a.add("def");
    a.add("ghi");

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
    let rope = Rope::from("abc");
    let rope = rope.byte_slice(3..3);
    assert_eq!(rope.to_string(), "".to_string())
  }

  #[test]
  #[should_panic]
  fn slice_panics_range_start_out_of_bounds() {
    let mut a = Rope::new();
    a.add("abc");
    a.byte_slice(3..4);
  }

  #[test]
  #[should_panic]
  fn slice_panics_range_start_greater_than_end() {
    let mut a = Rope::new();
    a.add("abc");
    a.byte_slice(1..0);
  }

  #[test]
  #[should_panic]
  fn slice_panics_range_end_out_of_bounds() {
    let mut a = Rope::new();
    a.add("abc");
    a.byte_slice(0..4);
  }

  #[test]
  fn eq() {
    let mut a = Rope::new();
    a.add("abc");
    a.add("def");
    a.add("ghi");
    assert_eq!(&a, "abcdefghi");
    assert_eq!(a, "abcdefghi");

    let mut b = Rope::new();
    b.add("abcde");
    b.add("fghi");

    assert_eq!(a, b);
  }

  #[test]
  fn from() {
    let _ = Rope::from("abc");
    let _ = Rope::from("abc");
    let rope = Rope::from_iter(["abc", "def"]);
    assert_eq!(rope, "abcdef");
    assert_eq!(
      rope.repr,
      Repr::Complex(Rc::new(Vec::from_iter([("abc", 0), ("def", 3)])))
    );
  }

  #[test]
  fn byte() {
    let mut a = Rope::from("abc");
    assert_eq!(a.byte(0), b'a');
    a.add("d");
    assert_eq!(a.byte(3), b'd');
  }

  #[test]
  fn char_indices() {
    let mut a = Rope::new();
    a.add("abc");
    a.add("def");
    assert_eq!(
      a.char_indices().collect::<Vec<_>>(),
      "abcdef".char_indices().collect::<Vec<_>>()
    );

    let mut a = Rope::new();
    a.add("こんにちは");
    assert_eq!(
      a.char_indices().collect::<Vec<_>>(),
      "こんにちは".char_indices().collect::<Vec<_>>()
    );
    a.add("世界");
    assert_eq!(
      a.char_indices().collect::<Vec<_>>(),
      "こんにちは世界".char_indices().collect::<Vec<_>>()
    );
  }

  #[test]
  fn lines1() {
    let rope = Rope::from("abc");
    let lines = rope.lines().collect::<Vec<_>>();
    assert_eq!(lines, ["abc"]);

    // empty line at the end if the line before ends with a newline ('\n')
    let rope = Rope::from("abc\ndef\n");
    let lines = rope.lines().collect::<Vec<_>>();
    assert_eq!(lines, ["abc\n", "def\n", ""]);

    // no empty line at the end if the line before does not end with a newline ('\n')
    let rope = Rope::from("abc\ndef");
    let lines = rope.lines().collect::<Vec<_>>();
    assert_eq!(lines, ["abc\n", "def"]);

    let rope = Rope::from("Test\nTest\nTest\n");
    let lines = rope.lines().collect::<Vec<_>>();
    assert_eq!(lines, ["Test\n", "Test\n", "Test\n", ""]);

    let rope = Rope::from("\n");
    let lines = rope.lines().collect::<Vec<_>>();
    assert_eq!(lines, ["\n", ""]);

    let rope = Rope::from("\n\n");
    let lines = rope.lines().collect::<Vec<_>>();
    assert_eq!(lines, ["\n", "\n", ""]);
  }

  #[test]
  fn lines2() {
    let rope = Rope::from_iter(["abc\n", "def\n", "ghi\n"]);
    let lines = rope.lines().collect::<Vec<_>>();
    // empty line at the end if the line before ends with a newline ('\n')
    assert_eq!(lines, ["abc\n", "def\n", "ghi\n", ""]);

    let rope = Rope::from_iter(["abc\n", "def\n", "ghi"]);
    let lines = rope.lines().collect::<Vec<_>>();
    assert_eq!(lines, ["abc\n", "def\n", "ghi"]);

    let rope = Rope::from_iter(["abc\ndef", "ghi\n", "jkl"]);
    let lines = rope.lines().collect::<Vec<_>>();
    assert_eq!(lines, ["abc\n", "defghi\n", "jkl"]);

    let rope = Rope::from_iter(["a\nb", "c\n", "d\n"]);
    let lines = rope.lines().collect::<Vec<_>>();
    assert_eq!(lines, ["a\n", "bc\n", "d\n", ""]);

    let rope = Rope::from_iter(["\n"]);
    let lines = rope.lines().collect::<Vec<_>>();
    assert_eq!(lines, ["\n", ""]);
  }

  #[test]
  fn lines_with_trailing_line_break_as_newline() {
    let trailing_line_break_as_newline = false;
    let rope = Rope::from("abc\n");
    let lines = rope
      .lines_impl(trailing_line_break_as_newline)
      .collect::<Vec<_>>();
    assert_eq!(lines, ["abc\n"]);

    let rope = Rope::from("\n");
    let lines = rope
      .lines_impl(trailing_line_break_as_newline)
      .collect::<Vec<_>>();
    assert_eq!(lines, ["\n"]);
  }
}
