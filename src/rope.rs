#![allow(unsafe_code)]

use std::{
  borrow::Cow, hash::Hash, ops::{self, AddAssign, Bound, RangeBounds}, rc::Rc
};

use crate::{sum_tree::{self, SumTree}, Error};

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct TextSummary {
  /// Length in bytes.
  pub len: usize,
}

impl sum_tree::Item for &str {
    type Summary = TextSummary;

    fn summary(&self, _cx: <Self::Summary as sum_tree::Summary>::Context<'_>) -> Self::Summary {
        TextSummary {
                len: self.len(),
            }
    }
}

impl<'a> From<&'a str> for TextSummary {
    fn from(text: &'a str) -> Self {
        TextSummary {
            len: text.len(),
        }
    }
}

impl sum_tree::ContextLessSummary for TextSummary {
    fn zero() -> Self {
        Default::default()
    }

    fn add_summary(&mut self, summary: &Self) {
        *self += summary;
    }
}

impl ops::Add<Self> for TextSummary {
    type Output = Self;

    fn add(mut self, rhs: Self) -> Self::Output {
        AddAssign::add_assign(&mut self, &rhs);
        self
    }
}

impl<'a> ops::AddAssign<&'a Self> for TextSummary {
    fn add_assign(&mut self, other: &'a Self) {
        self.len += other.len;
    }
}

impl<'a> sum_tree::Dimension<'a, TextSummary> for usize {
    fn zero(_cx: ()) -> Self {
        Default::default()
    }

    fn add_summary(&mut self, summary: &'a TextSummary, _: ()) {
        *self += summary.len;
    }
}

#[derive(Clone, Debug)]
pub(crate) enum Repr<'a> {
  Light(&'a str),
  Full(SumTree<&'a str>),
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
  pub fn push(&mut self, text: &'a str) {
    if text.is_empty() {
      return;
    }
    match &mut self.repr {
      Repr::Light(s) => {
        if s.is_empty() {
          *s = text;
          return;
        }
        let chunks = SumTree::from_iter([*s, text], ());
        self.repr = Repr::Full(chunks);
      }
      Repr::Full(chunks) => {
        chunks.push(text, ());
      }
    }
  }

  /// Appends another rope to this rope.
  ///
  /// Handles all combinations of simple and complex representations efficiently.
  pub fn append(&mut self, rope: Rope<'a>) {
    match (&mut self.repr, rope.repr) {
      (Repr::Light(s), Repr::Light(other)) => {
        if other.is_empty() {
          return;
        }
        if s.is_empty() {
          *s = other;
          return;
        }
        let chunks = SumTree::from_iter([*s, other], ());
        self.repr = Repr::Full(chunks);
      }
      (Repr::Full(s), Repr::Full(other)) => {
        if other.is_empty() {
          return;
        }
        s.append(other, ());
      }
      (Repr::Full(s), Repr::Light(other)) => {
        if other.is_empty() {
          return;
        }
        s.push(other, ());
      }
      (Repr::Light(s), Repr::Full(other)) => {
        if s.is_empty() {
          self.repr = Repr::Full(other);
          return;
        }
        let mut chunks = SumTree::default();
        chunks.push(*s, ());
        chunks.append(other, ());
        self.repr = Repr::Full(chunks);
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
      Repr::Full(chunks) => {
        let mut cursor = chunks.cursor::<usize>(());
        cursor.seek(&byte_index, sum_tree::Bias::Right);
        if let Some(chunk) = cursor.item()
        {
          Some(chunk.as_bytes()[byte_index - *cursor.start()])
        } else {
          None
        }
      }
    }
  }

  /// Returns an iterator over the characters and their byte positions.
  pub fn char_indices(&self) -> CharIndices<'_, 'a> {
    match &self.repr {
      Repr::Light(s) => CharIndices {
        iter: CharIndicesEnum::Light {
          iter: s.char_indices(),
        },
      },
      Repr::Full(chunks) => {
        CharIndices {
          iter: CharIndicesEnum::Full {
            chunks: chunks.iter(),
            iter: None,
            start_pos: 0,
            chunk_len: 0,
          },
        }
      },
    }
  }

  /// Returns whether the rope starts with the given string.
  pub fn starts_with(&self, value: &Rope) -> bool {
    match &self.repr {
      Repr::Light(s) => match &value.repr {
        Repr::Light(other) => s.starts_with(other),
        Repr::Full(chunks) => {
          let mut remaining = *s;
          for chunk in chunks.iter() {
            if remaining.starts_with(chunk) {
              remaining = &remaining[chunk.len()..];
            } else {
              return false;
            }
          }
          remaining.is_empty()
        }
      },
      Repr::Full(chunks) => {
        match &value.repr {
          Repr::Light(other) => {
            // Check if the concatenated chunks of `data` start with `other`
            let mut remaining_other = *other;
            for chunk in chunks.iter() {
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
            let mut self_iter = chunks.iter();
            let mut other_iter = other_data.iter();

            let mut remaining_self = "";
            let mut remaining_other = "";

            loop {
              // If `remaining_other` is empty, try to fill it with the next chunk from `other_data`
              if remaining_other.is_empty() {
                if let Some(other_chunk) = other_iter.next() {
                  remaining_other = other_chunk;
                } else {
                  // If there are no more chunks in `other_data`, we have matched everything
                  return true;
                }
              }

              // If `remaining_self` is empty, try to fill it with the next chunk from `data`
              if remaining_self.is_empty() {
                if let Some(self_chunk) = self_iter.next() {
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
      Repr::Full(chunks) => {
        if let Some(chunk) = chunks.last() {
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
      Repr::Full(chunks) => chunks.is_empty(),
    }
  }

  /// Returns the length of the rope in bytes.
  #[inline]
  pub fn len(&self) -> usize {
    match &self.repr {
      Repr::Light(s) => s.len(),
      Repr::Full(chunks) =>  {
        chunks.extent(())
      }
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
      Repr::Full(chunks) => {
        // find the entry that may contain start_range (the largest key <= start_range)
        let mut cursor = chunks.cursor::<usize>(());
        cursor.seek_forward(&start_range, sum_tree::Bias::Right);

        let mut slice = SumTree::default();

        loop {
          if let Some(chunk) = cursor.item() {
            let chunk_len = chunk.len();
            let chunk_start = *cursor.start();
            let chunk_end = chunk_start + chunk_len;

            cursor.next();

            // skip chunks entirely before the requested range
            if chunk_end <= start_range {
              continue;
            }
            // stop once we've passed the requested end
            if chunk_start >= end_range {
              break;
            }

            // compute local slice bounds within this chunk
            let s = if start_range > chunk_start {
              start_range - chunk_start
            } else {
              0
            };
            let e = if end_range < chunk_end {
              end_range - chunk_start
            } else {
              chunk_len
            };

            // validate char boundaries and insert
            if let Some(sub) = chunk.get(s..e) {
              slice.push(sub, ());
            } else {
              return Err(Error::Rope("invalid char boundary"));
            }
          } else {
            break;
          }
        }

        if slice.is_empty() {
          return Ok(Rope::new());
        }

        Ok(Rope {
          repr: Repr::Full(slice),
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
      Repr::Full(chunks) => {
        let mut bytes = vec![];
        for chunk in chunks.iter() {
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
      Repr::Full(chunks) => {
        for chunk in chunks.iter() {
          chunk.hash(state);
        }
      }
    }
  }
}

enum LinesEnum<'chunks, 'text> {
  Light(&'text str),
  Complex {
    chunks: sum_tree::Iter<'chunks, &'text str>,
    chunk: Option<&'text str>,
    in_chunk_byte_idx: usize,
  },
}

pub struct Lines<'chunks, 'text> {
  iter: LinesEnum<'chunks, 'text>,
  byte_idx: usize,
  ended: bool,
  total_bytes: usize,

  /// Whether to treat the end of the rope with ('\n') as an empty newline.
  trailing_line_break_as_newline: bool,
}

impl<'chunks, 'text> Iterator for Lines<'chunks, 'text> {
  type Item = Rope<'text>;

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
          *chunk = chunks.next().map(|c| *c);
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
        let mut new_chunks = SumTree::default();
        let mut len = 0usize;

        // push remainder of current chunk
        new_chunks.push(&cur[*in_chunk_byte_idx..], ());
        len += cur.len() - *in_chunk_byte_idx;

        // consume current chunk
        *chunk = None;
        *in_chunk_byte_idx = 0;

        while let Some(next_chunk) = chunks.next() {
          if next_chunk.is_empty() {
            continue;
          }
          if let Some(idx) = memchr::memchr(b'\n', next_chunk.as_bytes()) {
            // include up to and including newline
            new_chunks.push(&next_chunk[..idx + 1], ());
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
              repr: Repr::Full(new_chunks),
            });
          } else {
            new_chunks.push(next_chunk, ());
            len += next_chunk.len();
            // continue looking
          }
        }

        // Reached end without finding newline
        self.ended = true;
        self.byte_idx += len;
        if new_chunks.is_empty() {
          return None;
        }
        // If only a single piece and it spans the remainder of a single original chunk,
        // it's fine to return a Light Rope but keeping Full is OK and consistent with other code.
        Some(Rope {
          repr: Repr::Full(new_chunks),
        })
      }
    }
  }
}

enum CharIndicesEnum<'chunks, 'text> {
  Light {
    iter: std::str::CharIndices<'text>,
  },
  Full {
    chunks: sum_tree::Iter<'chunks, &'text str>,
    iter: Option<std::str::CharIndices<'text>>,
    start_pos: usize,
    chunk_len: usize,
  },
}

pub struct CharIndices<'chunks, 'text> {
  iter: CharIndicesEnum<'chunks, 'text>,
}

impl Iterator for CharIndices<'_, '_> {
  type Item = (usize, char);

  fn next(&mut self) -> Option<Self::Item> {
    match &mut self.iter {
      CharIndicesEnum::Light { iter } => iter.next(),
      CharIndicesEnum::Full {
        chunks,
        iter,
        chunk_len,
        start_pos,
      } => {
        // try current chunk iterator first
        if let Some(inner) = iter.as_mut() {
          if let Some((i, c)) = inner.next() {
            return Some((*start_pos + i, c));
          }
        }

        // advance to next chunk from the BTreeMap iterator
        while let Some(chunk) = chunks.next() {
          if chunk.is_empty() {
            continue;
          }

          let mut new_iter = chunk.char_indices();
          *start_pos += *chunk_len;
          *chunk_len = chunk.len();
          if let Some((i, c)) = new_iter.next() {
            *iter = Some(new_iter);
            return Some((*start_pos + i, c));
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
      Repr::Full(chunks) => {
        let mut s = String::with_capacity(self.len());
        for chunk in chunks.iter() {
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

    let mut chunks: Box<dyn Iterator<Item = &&str>> = match &self.repr {
      Repr::Light(s) => Box::new([s].into_iter()),
      Repr::Full(chunks) => Box::new(chunks.iter()),
    };
    let mut other_chunks: Box<dyn Iterator<Item = &&str>> = match &other.repr {
      Repr::Light(s) => Box::new([s].into_iter()),
      Repr::Full(chunks) => Box::new(chunks.iter()),
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
      Repr::Full(chunks) => {
        let mut idx = 0;
        for chunk in chunks.iter() {
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
      Repr::Full(chunks) => {
        let mut idx = 0;
        for chunk in chunks.iter() {
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
   let mut rope = Rope::new();
    for chunk in iter {
        rope.push(chunk);
    }
    rope
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
  use crate::{rope::{Repr, Rope}, sum_tree::SumTree};

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
  fn push() {
    let mut simple = Rope::from("abc");
    assert_eq!(simple.repr, Repr::Light("abc"));
    assert_eq!(simple.len(), 3);

    simple.push("def");
    assert_eq!(simple, "abcdef");
    assert_eq!(
      simple.repr,
      Repr::Full(SumTree::from_iter(["abc", "def"], ()))
    );
    assert_eq!(simple.len(), 6);

    simple.push("ghi");
    assert_eq!(simple, "abcdefghi");
    assert_eq!(
      simple.repr,
      Repr::Full(SumTree::from_iter([
        "abc",
        "def",
        "ghi",
      ], ()))
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
      Repr::Full(SumTree::from_iter(["abc", "def"], ()))
    );

    // simple - complex
    let mut append2 = simple1.clone();
    append2.append(complex1.clone());
    assert_eq!(append2, "abc123");
    assert_eq!(
      append2.repr,
      Repr::Full(SumTree::from_iter([
        "abc",
        "1",
        "2",
        "3",
      ], ()))
    );

    // complex - simple
    let mut append3 = complex1.clone();
    append3.append(simple1.clone());
    assert_eq!(append3, "123abc");
    assert_eq!(
      append3.repr,
      Repr::Full(SumTree::from_iter([
        "1",
        "2",
        "3",
        "abc",
      ], ()))
    );

    // complex - complex
    let mut append4 = complex1.clone();
    append4.append(complex2.clone());
    assert_eq!(append4, "123456");
    assert_eq!(
      append4.repr,
      Repr::Full(SumTree::from_iter([
        "1",
        "2",
        "3",
        "4",
        "5",
        "6",
      ], ()))
    );
  }

  #[test]
  fn slice() {
    let mut a = Rope::new();
    a.push("abc");
    a.push("def");
    a.push("ghi");

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
    a.push("abc");
    a.byte_slice(3..4);
  }

  #[test]
  #[should_panic]
  fn slice_panics_range_start_greater_than_end() {
    let mut a = Rope::new();
    a.push("abc");
    a.byte_slice(1..0);
  }

  #[test]
  #[should_panic]
  fn slice_panics_range_end_out_of_bounds() {
    let mut a = Rope::new();
    a.push("abc");
    a.byte_slice(0..4);
  }

  #[test]
  fn eq() {
    let mut a = Rope::new();
    a.push("abc");
    a.push("def");
    a.push("ghi");
    assert_eq!(&a, "abcdefghi");
    assert_eq!(a, "abcdefghi");

    let mut b = Rope::new();
    b.push("abcde");
    b.push("fghi");

    assert_eq!(a, b);

    let mut a = Rope::new();
    a.push("abc");

    let mut b = Rope::new();
    b.push("a");
    b.push("b");
    b.push("c");

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
      Repr::Full(SumTree::from_iter(["abc", "def"], ()))
    );
  }

  #[test]
  fn byte() {
    let mut a = Rope::from("abc");
    assert_eq!(a.byte(0), b'a');
    a.push("d");
    assert_eq!(a.byte(3), b'd');
  }

  #[test]
  fn char_indices() {
    let mut a = Rope::new();
    a.push("abc");
    a.push("def");
    assert_eq!(
      a.char_indices().collect::<Vec<_>>(),
      "abcdef".char_indices().collect::<Vec<_>>()
    );

    let mut a = Rope::new();
    a.push("こんにちは");
    assert_eq!(
      a.char_indices().collect::<Vec<_>>(),
      "こんにちは".char_indices().collect::<Vec<_>>()
    );
    a.push("世界");
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
