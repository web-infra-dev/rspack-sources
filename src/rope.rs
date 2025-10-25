#![allow(unsafe_code)]

use std::{
  borrow::Cow, cell::Cell, collections::{btree_map, BTreeMap}, hash::Hash, ops::{Bound, RangeBounds}, rc::Rc
};

use crate::Error;

#[derive(Clone, Debug)]
pub(crate) enum Repr<'a> {
  Light(&'a str),
  Full(Rc<BTreeMap<Cell<usize>, &'a str>>),
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
      repr: Repr::Light(""),
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
      Repr::Light(s) => {
        let root = BTreeMap::from_iter([(Cell::new(0), *s), (Cell::new(s.len()), value)]);
        self.repr = Repr::Full(Rc::new(root));
      }
      Repr::Full(data) => {
        let len = data.last_key_value()
          .map_or(0, |(start_pos, chunk)| start_pos.get() + chunk.len());
        Rc::make_mut(data).insert(Cell::new(len), value);
      }
    }
  }

  /// Appends another rope to this rope.
  ///
  /// Handles all combinations of simple and complex representations efficiently.
  pub fn append(&mut self, value: Rope<'a>) {
    match (&mut self.repr, value.repr) {
      (Repr::Light(s), Repr::Light(other)) => {
        if other.is_empty() {
          return;
        }
        let raw = BTreeMap::from_iter([(Cell::new(0), *s), (Cell::new(s.len()), other)]);
        self.repr = Repr::Full(Rc::new(raw));
      }
      (Repr::Full(s), Repr::Full(mut other)) => {
        if other.is_empty() {
          return;
        }
        let mut len = s.last_key_value()
          .map_or(0, |(start_pos, chunk)| {
            start_pos.get() + chunk.len()
          });

        let other = Rc::make_mut(&mut other);
        for (start_pot, chunk) in other.iter() {
          start_pot.set(len);
          len += chunk.len();
        }

        let cur: &mut BTreeMap<Cell<usize>, &str> = Rc::make_mut(s);
        cur.append(other);
      }
      (Repr::Full(s), Repr::Light(other)) => {
        if other.is_empty() {
          return;
        }
        let len = s.last_key_value()
          .map_or(0, |(start_pos, chunk)| start_pos.get() + chunk.len());
        Rc::make_mut(s).insert(Cell::new(len), other);
      }
      (Repr::Light(s), Repr::Full(other)) => {
        if s.is_empty() {
          self.repr = Repr::Full(other.clone());
          return;
        }
        let mut raw = BTreeMap::new();
        raw.insert(Cell::new(0), *s);
        let mut len = s.len();
        for (_, chunk) in other.iter() {
          raw.insert(Cell::new(len), chunk);
          len += chunk.len();
        }
        self.repr = Repr::Full(Rc::new(raw));
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
      Repr::Light(s) => Some(s.as_bytes()[byte_index]),
      Repr::Full(tree) => {
        if let Some((start_pos, chunk)) = tree.range(Cell::new(byte_index)..).next()
        {
          let pos = byte_index - start_pos.get();
          Some(chunk.as_bytes()[pos])
        } else {
          None
        }
      }
    }
  }

  /// Returns an iterator over the characters and their byte positions.
  pub fn char_indices(&self) -> CharIndices<'_> {
    match &self.repr {
      Repr::Light(s) => CharIndices {
        iter: CharIndicesEnum::Light {
          iter: s.char_indices(),
        },
      },
      Repr::Full(tree) => CharIndices {
        iter: CharIndicesEnum::Full {
          chunks: tree.iter(),
          iter: None,
          start_pos: 0,
        },
      },
    }
  }

  /// Returns whether the rope starts with the given string.
  pub fn starts_with(&self, value: &Rope) -> bool {
    match &self.repr {
      Repr::Light(s) => match &value.repr {
        Repr::Light(other) => s.starts_with(other),
        Repr::Full(data) => {
          let mut remaining = *s;
          for (_, chunk) in data.iter() {
            if remaining.starts_with(chunk) {
              remaining = &remaining[chunk.len()..];
            } else {
              return false;
            }
          }
          remaining.is_empty()
        }
      },
      Repr::Full(data) => {
        match &value.repr {
          Repr::Light(other) => {
            // Check if the concatenated chunks of `data` start with `other`
            let mut remaining_other = *other;
            for (_, chunk) in data.iter() {
              if remaining_other.is_empty() {
                return true;
              }
              if chunk.starts_with(remaining_other) {
                return true;
              }
              if remaining_other.starts_with(chunk) {
                remaining_other = &remaining_other[chunk.len()..];
              } else {
                return false;
              }
            }
            remaining_other.is_empty()
          }
          Repr::Full(other_data) => {
            // Iterate through both `data` and `other_data` to check if `data` starts with `other_data`
            let mut self_iter = data.iter();
            let mut other_iter = other_data.iter();

            let mut remaining_self = "";
            let mut remaining_other = "";

            loop {
              // If `remaining_other` is empty, try to fill it with the next chunk from `other_data`
              if remaining_other.is_empty() {
                if let Some((_, other_chunk)) = other_iter.next() {
                  remaining_other = other_chunk;
                } else {
                  // If there are no more chunks in `other_data`, we have matched everything
                  return true;
                }
              }

              // If `remaining_self` is empty, try to fill it with the next chunk from `data`
              if remaining_self.is_empty() {
                if let Some((_, self_chunk)) = self_iter.next() {
                  remaining_self = self_chunk;
                } else {
                  // If there are no more chunks in `data`, but `other_data` still has chunks, it cannot match
                  return false;
                }
              }

              // Compare the remaining parts
              let min_len = remaining_self.len().min(remaining_other.len());
              if remaining_self[..min_len] != remaining_other[..min_len] {
                return false;
              }

              // Remove the compared parts
              remaining_self = &remaining_self[min_len..];
              remaining_other = &remaining_other[min_len..];
            }
          }
        }
      }
    }
  }

  /// Returns whether the rope ends with the given string.
  #[inline]
  pub fn ends_with(&self, value: char) -> bool {
    match &self.repr {
      Repr::Light(s) => s.ends_with(value),
      Repr::Full(tree) => {
        if let Some((_, chunk)) = tree.last_key_value() {
          chunk.ends_with(value)
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
      Repr::Light(s) => s.is_empty(),
      Repr::Full(data) => data.iter().all(|(_, s)| s.is_empty()),
    }
  }

  /// Returns the length of the rope in bytes.
  #[inline]
  pub fn len(&self) -> usize {
    match &self.repr {
      Repr::Light(s) => s.len(),
      Repr::Full(tree) => tree.last_key_value()
        .map_or(0, |(start_pos, chunk)| start_pos.get() + chunk.len()),
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
      Repr::Light(s) => s
        .get(start_range..end_range)
        .map(Rope::from)
        .ok_or(Error::Rope("invalid char boundary")),
      Repr::Full(tree) => {
        // find the entry that may contain start_range (the largest key <= start_range)
        let start_key = tree.range(..=Cell::new(start_range)).next_back().map(|(k, _)| k.clone());
        let iter = match start_key {
          Some(k) => tree.range(k..),
          None => tree.range(Cell::new(start_range)..),
        };

        let mut slice = BTreeMap::new();
        let mut len = 0usize;

        for (ref chunk_start, &chunk) in iter {
          let chunk_len = chunk.len();
          let chunk_end = chunk_start.get() + chunk_len;

          // skip chunks entirely before the requested range
          if chunk_end <= start_range {
            continue;
          }
          // stop once we've passed the requested end
          if chunk_start.get() >= end_range {
            break;
          }

          // compute local slice bounds within this chunk
          let s = if start_range > chunk_start.get() {
            start_range - chunk_start.get()
          } else {
            0
          };
          let e = if end_range < chunk_end {
            end_range - chunk_start.get()
          } else {
            chunk_len
          };

          // validate char boundaries and insert
          if let Some(sub) = chunk.get(s..e) {
            slice.insert(Cell::new(len), sub);
            len += sub.len();
          } else {
            return Err(Error::Rope("invalid char boundary"));
          }
        }

        if slice.is_empty() {
          return Ok(Rope::new());
        }

        Ok(Rope {
          repr: Repr::Full(Rc::new(slice)),
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
      Repr::Light(s) => {
        // SAFETY: invariant guarantees valid range
        Rope::from(unsafe { s.get_unchecked(start_range..end_range) })
      }
      Repr::Full(data) => {
        todo!();
        // // [start_chunk
        // let start_chunk_index = data
        //   .binary_search_by(|(_, start_pos)| start_pos.cmp(&start_range))
        //   .unwrap_or_else(|insert_pos| insert_pos.saturating_sub(1));

        // // end_chunk)
        // let end_chunk_index = data
        //   .binary_search_by(|(chunk, start_pos)| {
        //     let end_pos = start_pos + chunk.len(); // exclusive
        //     end_pos.cmp(&end_range)
        //   })
        //   .unwrap_or_else(|insert_pos| insert_pos);

        // // same chunk
        // if start_chunk_index == end_chunk_index {
        //   // SAFETY: start_chunk_index guarantees valid range
        //   let (chunk, start_pos) =
        //     unsafe { data.get_unchecked(start_chunk_index) };
        //   let start = start_range - start_pos;
        //   let end = end_range - start_pos;
        //   // SAFETY: invariant guarantees valid range
        //   return Rope::from(unsafe { chunk.get_unchecked(start..end) });
        // }

        // if end_chunk_index < start_chunk_index {
        //   return Rope::new();
        // }

        // let mut raw =
        //   Vec::with_capacity(end_chunk_index - start_chunk_index + 1);
        // let mut len = 0;

        // // different chunk
        // // [start_chunk, end_chunk]
        // (start_chunk_index..end_chunk_index + 1).for_each(|i| {
        //   // SAFETY: [start_chunk_index, end_chunk_index] guarantees valid range
        //   let (chunk, start_pos) = unsafe { data.get_unchecked(i) };

        //   if start_chunk_index == i {
        //     let start = start_range - start_pos;
        //     // SAFETY: invariant guarantees valid range
        //     let chunk = unsafe { chunk.get_unchecked(start..) };
        //     raw.push((chunk, len));
        //     len += chunk.len();
        //   } else if end_chunk_index == i {
        //     let end = end_range - start_pos;
        //     // SAFETY: invariant guarantees valid range
        //     let chunk = unsafe { chunk.get_unchecked(..end) };
        //     raw.push((chunk, len));
        //     len += chunk.len();
        //   } else {
        //     raw.push((chunk, len));
        //     len += chunk.len();
        //   }
        // });

        // Rope {
        //   repr: Repr::Full(Rc::new(raw)),
        // }
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
        Repr::Light(s) => LinesEnum::Light(s),
        Repr::Full(tree) => LinesEnum::Complex {
          chunks: tree.iter(),
          chunk: None,
          in_chunk_byte_idx: 0,
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
      Repr::Light(s) => Cow::Borrowed(s.as_bytes()),
      Repr::Full(data) => {
        let mut bytes = vec![];
        for (_, chunk) in data.iter() {
          bytes.extend_from_slice(chunk.as_bytes());
        }
        Cow::Owned(bytes)
      }
    }
  }
}

impl Hash for Rope<'_> {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    match &self.repr {
      Repr::Light(s) => s.hash(state),
      Repr::Full(data) => {
        for (s, _) in data.iter() {
          s.get().hash(state);
        }
      }
    }
  }
}

enum LinesEnum<'iter, 'str> {
  Light(&'str str),
  Complex {
    chunks: btree_map::Iter<'iter, Cell<usize>, &'str str>,
    chunk: Option<&'str str>,
    in_chunk_byte_idx: usize,
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
        iter: LinesEnum::Light(s),
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
            ref mut chunks,
            ref mut chunk,
            ref mut in_chunk_byte_idx,
          },
        ref mut byte_idx,
        ref mut ended,
        ref total_bytes,
        trailing_line_break_as_newline,
      } => {
        if self.ended {
          return None;
        } else if self.byte_idx == self.total_bytes {
          if self.trailing_line_break_as_newline {
            self.ended = true;
            return Some(Rope::from(""));
          }
          return None;
        }

        // Ensure we have a current chunk
        if chunk.is_none() {
          *chunk = chunks.next().map(|(_, c)| *c);
          *in_chunk_byte_idx = 0;
        }

        // If still no chunk, nothing to return
        let cur = match chunk {
          Some(c) => *c,
          None => return None,
        };

        // Fast-path: newline in current chunk
        if let Some(idx) =
          memchr::memchr(b'\n', &cur.as_bytes()[*in_chunk_byte_idx..])
        {
          let end = *in_chunk_byte_idx + idx + 1;
          let result = Rope::from(&cur[*in_chunk_byte_idx..end]);
          self.byte_idx += end - *in_chunk_byte_idx;
          *in_chunk_byte_idx = end;
          // if we've consumed the chunk, drop it so next call advances iterator
          if *in_chunk_byte_idx == cur.len() {
            *chunk = None;
            *in_chunk_byte_idx = 0;
          }
          return Some(result);
        }

        // No newline in current chunk: collect across chunks until newline or end
        let mut raw: BTreeMap<Cell<usize>, &str> = BTreeMap::default();
        let mut len = 0usize;

        // push remainder of current chunk
        raw.insert(Cell::new(len), &cur[*in_chunk_byte_idx..]);
        len += cur.len() - *in_chunk_byte_idx;

        // consume current chunk
        *chunk = None;
        *in_chunk_byte_idx = 0;

        while let Some((_, next_chunk)) = chunks.next() {
          if next_chunk.is_empty() {
            continue;
          }
          if let Some(idx) = memchr::memchr(b'\n', next_chunk.as_bytes()) {
            // include up to and including newline
            raw.insert(Cell::new(len), &next_chunk[..idx + 1]);
            len += idx + 1;
            // set current chunk to remainder after newline (may be empty)
            if idx + 1 < next_chunk.len() {
              *chunk = Some(&next_chunk[idx + 1..]);
              *in_chunk_byte_idx = 0;
            } else {
              *chunk = None;
              *in_chunk_byte_idx = 0;
            }
            self.byte_idx += len;
            return Some(Rope {
              repr: Repr::Full(Rc::new(raw)),
            });
          } else {
            raw.insert(Cell::new(len), next_chunk);
            len += next_chunk.len();
            // continue looking
          }
        }

        // Reached end without finding newline
        self.ended = true;
        self.byte_idx += len;
        if raw.is_empty() {
          return None;
        }
        // If only a single piece and it spans the remainder of a single original chunk,
        // it's fine to return a Light Rope but keeping Full is OK and consistent with other code.
        Some(Rope {
          repr: Repr::Full(Rc::new(raw)),
        })
      }
    }
  }
}

enum CharIndicesEnum<'a> {
  Light {
    iter: std::str::CharIndices<'a>,
  },
  Full {
    chunks: btree_map::Iter<'a, Cell<usize>, &'a str>,
    iter: Option<std::str::CharIndices<'a>>,
    start_pos: usize,
  },
}

pub struct CharIndices<'a> {
  iter: CharIndicesEnum<'a>,
}

impl Iterator for CharIndices<'_> {
  type Item = (usize, char);

  fn next(&mut self) -> Option<Self::Item> {
    match &mut self.iter {
      CharIndicesEnum::Light { iter } => iter.next(),
      CharIndicesEnum::Full {
        chunks,
        iter,
        start_pos,
      } => {
        // try current chunk iterator first
        if let Some(inner) = iter.as_mut() {
          if let Some((i, c)) = inner.next() {
            return Some((*start_pos + i, c));
          }
        }

        // advance to next chunk from the BTreeMap iterator
        while let Some((key, chunk)) = chunks.next() {
          let key = key;
          if chunk.is_empty() {
            continue;
          }

          let mut new_iter = chunk.char_indices();
          *start_pos = key.get();
          if let Some((i, c)) = new_iter.next() {
            *iter = Some(new_iter);
            return Some((key.get() + i, c));
          } else {
            // empty after decoding (shouldn't happen for non-empty chunk,
            // but keep iter state consistent)
            *iter = Some(new_iter);
            continue;
          }
        }

        None
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
      Repr::Light(s) => s.to_string(),
      Repr::Full(data) => {
        let mut s = String::with_capacity(self.len());
        for (_, chunk) in data.iter() {
          s.push_str(chunk);
        }
        s
      }
    }
  }
}

impl PartialEq<Rope<'_>> for Rope<'_> {
  fn eq(&self, other: &Rope<'_>) -> bool {
    // fast path: different lengths
    if self.len() != other.len() {
      return false;
    }

    if let (
      Rope {
        repr: Repr::Light(s),
      },
      Rope {
        repr: Repr::Light(other),
      },
    ) = (self, other)
    {
      return s == other;
    }

    let mut chunks: Box<dyn Iterator<Item = &str>> = match &self.repr {
      Repr::Light(s) => Box::new([*s].into_iter()),
      Repr::Full(tree) => Box::new(tree.iter().map(|(_, chunk)| *chunk)),
    };
    let mut other_chunks: Box<dyn Iterator<Item = &str>> = match &other.repr {
      Repr::Light(s) => Box::new([*s].into_iter()),
      Repr::Full(tree) => Box::new(tree.iter().map(|(_, chunk)| *chunk)),
    };

    let mut chunk = chunks.next();
    let mut in_chunk_byte_idx = 0;

    let mut other_chunk = other_chunks.next();
    let mut in_other_chunk_byte_idx = 0;

    loop {
      let Some(chunk_str) = chunk else {
        return other_chunk.is_none();
      };
      let Some(other_chunk_str) = other_chunk else {
        return false;
      };

      let chunk_len = chunk_str.len();
      let other_chunk_len = other_chunk_str.len();

      let chunk_remaining = chunk_len - in_chunk_byte_idx;
      let other_chunk_remaining = other_chunk_len - in_other_chunk_byte_idx;

      match chunk_remaining.cmp(&other_chunk_remaining) {
        std::cmp::Ordering::Less => {
          if other_chunk_str
            [in_other_chunk_byte_idx..in_other_chunk_byte_idx + chunk_remaining]
            != chunk_str[in_chunk_byte_idx..]
          {
            return false;
          }
          in_other_chunk_byte_idx += chunk_remaining;
          chunk = chunks.next();
          in_chunk_byte_idx = 0;
        }
        std::cmp::Ordering::Equal => {
          if chunk_str[in_chunk_byte_idx..]
            != other_chunk_str[in_other_chunk_byte_idx..]
          {
            return false;
          }
          chunk = chunks.next();
          other_chunk = other_chunks.next();
          in_chunk_byte_idx = 0;
          in_other_chunk_byte_idx = 0;
        }
        std::cmp::Ordering::Greater => {
          if chunk_str[in_chunk_byte_idx..in_chunk_byte_idx + other_chunk_remaining]
            != other_chunk_str[in_other_chunk_byte_idx..]
          {
            return false;
          }
          in_chunk_byte_idx += other_chunk_remaining;
          other_chunk = other_chunks.next();
          in_other_chunk_byte_idx = 0;
        }
      }
    }
  }
}

impl PartialEq<str> for Rope<'_> {
  fn eq(&self, other: &str) -> bool {
    if self.len() != other.len() {
      return false;
    }

    let other = other.as_bytes();

    match &self.repr {
      Repr::Light(s) => {
        if s.as_bytes() != other {
          return false;
        }
      }
      Repr::Full(data) => {
        let mut idx = 0;
        for (_, chunk) in data.iter() {
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
      Repr::Light(s) => {
        if s.as_bytes() != other {
          return false;
        }
      }
      Repr::Full(data) => {
        let mut idx = 0;
        for (_, chunk) in data.iter() {
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
      repr: Repr::Light(value),
    }
  }
}

impl<'a> From<&'a String> for Rope<'a> {
  fn from(value: &'a String) -> Self {
    Rope {
      repr: Repr::Light(value),
    }
  }
}

impl<'a> From<&'a Cow<'a, str>> for Rope<'a> {
  fn from(value: &'a Cow<'a, str>) -> Self {
    Rope {
      repr: Repr::Light(value),
    }
  }
}

impl<'a> FromIterator<&'a str> for Rope<'a> {
  fn from_iter<T: IntoIterator<Item = &'a str>>(iter: T) -> Self {
    let mut len = 0;
    let tree = iter
      .into_iter()
      .filter_map(|chunk| {
        if chunk.is_empty() {
          return None;
        }
        let cur = (Cell::new(len), chunk);
        len += chunk.len();
        Some(cur)
      })
      .collect::<BTreeMap<_, _>>();

    Self {
      repr: Repr::Full(Rc::new(tree)),
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
  use std::{collections::BTreeMap, rc::Rc};

  use crate::rope::{Repr, Rope};

  impl<'a> PartialEq for Repr<'a> {
    fn eq(&self, other: &Self) -> bool {
      match (self, other) {
        (Repr::Light(a), Repr::Light(b)) => a == b,
        (Repr::Full(a), Repr::Full(b)) => a == b,
        _ => false,
      }
    }
  }

  impl<'a> Eq for Repr<'a> {}

  #[test]
  fn add() {
    let mut simple = Rope::from("abc");
    assert_eq!(simple.repr, Repr::Light("abc"));
    assert_eq!(simple.len(), 3);

    simple.add("def");
    assert_eq!(simple, "abcdef");
    assert_eq!(
      simple.repr,
      Repr::Full(Rc::new(BTreeMap::from_iter([(0.into(), "abc"), (3.into(), "def")])))
    );
    assert_eq!(simple.len(), 6);

    simple.add("ghi");
    assert_eq!(simple, "abcdefghi");
    assert_eq!(
      simple.repr,
      Repr::Full(Rc::new(BTreeMap::from_iter([
        (0.into(), "abc"),
        (3.into(), "def"),
        (6.into(), "ghi"),
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
      Repr::Full(Rc::new(BTreeMap::from_iter([(0.into(), "abc"), (3.into(), "def"),])))
    );

    // simple - complex
    let mut append2 = simple1.clone();
    append2.append(complex1.clone());
    assert_eq!(append2, "abc123");
    assert_eq!(
      append2.repr,
      Repr::Full(Rc::new(BTreeMap::from_iter([
        (0.into(), "abc"),
        (3.into(), "1"),
        (4.into(), "2"),
        (5.into(), "3"),
      ])))
    );

    // complex - simple
    let mut append3 = complex1.clone();
    append3.append(simple1.clone());
    assert_eq!(append3, "123abc");
    assert_eq!(
      append3.repr,
      Repr::Full(Rc::new(BTreeMap::from_iter([
        (0.into(), "1"),
        (1.into(), "2"),
        (2.into(), "3"),
        (3.into(), "abc"),
      ])))
    );

    // complex - complex
    let mut append4 = complex1.clone();
    append4.append(complex2.clone());
    assert_eq!(append4, "123456");
    assert_eq!(
      append4.repr,
      Repr::Full(Rc::new(BTreeMap::from_iter([
        (0.into(), "1"),
        (1.into(), "2"),
        (2.into(), "3"),
        (3.into(), "4"),
        (4.into(), "5"),
        (5.into(), "6"),
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

    let mut a = Rope::new();
    a.add("abc");

    let mut b = Rope::new();
    b.add("a");
    b.add("b");
    b.add("c");

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
      Repr::Full(Rc::new(BTreeMap::from_iter([(0.into(), "abc"), (3.into(), "def")])))
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

    let rope = Rope::from("abc");
    let lines = rope.lines().collect::<Vec<_>>();
    assert_eq!(lines, ["abc"]);
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

    let rope = Rope::from_iter(["a", "b", "c"]);
    let lines = rope.lines().collect::<Vec<_>>();
    assert_eq!(lines, ["abc"]);
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

  #[test]
  fn starts_with() {
    let rope = Rope::from("abc");
    assert!(rope.starts_with(&Rope::from("a")));
    assert!(rope.starts_with(&Rope::from("ab")));
    assert!(rope.starts_with(&Rope::from("abc")));
    assert!(!rope.starts_with(&Rope::from("abcd")));
    assert!(!rope.starts_with(&Rope::from("b")));
    assert!(!rope.starts_with(&Rope::from("bc")));
    assert!(!rope.starts_with(&Rope::from("c")));

    let rope = Rope::from_iter(vec!["a", "b", "c"]);
    assert!(rope.starts_with(&Rope::from("a")));
    assert!(rope.starts_with(&Rope::from("ab")));
    assert!(rope.starts_with(&Rope::from("abc")));
    assert!(!rope.starts_with(&Rope::from("abcd")));
    assert!(!rope.starts_with(&Rope::from("b")));
    assert!(!rope.starts_with(&Rope::from("bc")));
    assert!(!rope.starts_with(&Rope::from("c")));

    assert!(rope.starts_with(&Rope::from_iter(vec!["a", "b"])));
    assert!(rope.starts_with(&Rope::from_iter(vec!["a", "b", "c"])));
    assert!(!rope.starts_with(&Rope::from_iter(vec!["a", "b", "c", "d"])));
    assert!(!rope.starts_with(&Rope::from_iter(vec!["b", "c"])));
  }

  #[test]
  fn ends_with() {
    let rope = Rope::from("abc\n");
    assert!(rope.ends_with('\n'));

    let mut rope = Rope::from("abc\n");
    rope.append("".into());
    assert!(rope.ends_with('\n'));

    let rope = Rope::from_iter(["abc\n", ""]);
    assert!(rope.ends_with('\n'));

    let rope = Rope::from_iter(["abc", "\n"]);
    assert!(rope.ends_with('\n'));
  }
}
