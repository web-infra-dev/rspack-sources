use std::{
  fmt::Display,
  iter::once,
  ops::{Range, RangeBounds},
  slice::SliceIndex,
};

use crate::{
  helpers::{split, split_str},
  Rope,
};

use itertools::Either;

pub trait SourceText<'a>: Default + Clone + Display {
  fn split_into_lines(&self) -> impl Iterator<Item = Self>;
  fn len(&self) -> usize;
  fn ends_with(&self, value: &str) -> bool;
  fn char_indices(&self) -> impl Iterator<Item = (usize, char)>;
  fn byte_slice(&self, range: Range<usize>) -> Self;
  fn is_empty(&self) -> bool;
  fn into_rope(self) -> Rope<'a>
  where
    Self: Sized;
  fn get_byte(&self, byte_index: usize) -> Option<u8>;
}

impl<'a> SourceText<'a> for Rope<'a> {
  fn split_into_lines(&self) -> impl Iterator<Item = Self> {
    if let Some(s) = self.get_simple() {
      return Either::Left(split_str(s, b'\n').map(Rope::from_str));
    }
    Either::Right(split(self, b'\n'))
  }

  fn len(&self) -> usize {
    self.len()
  }

  fn ends_with(&self, value: &str) -> bool {
    (*self).ends_with(value)
  }

  fn char_indices(&self) -> impl Iterator<Item = (usize, char)> {
    self.char_indices()
  }

  fn byte_slice(&self, range: Range<usize>) -> Self {
    self.byte_slice(range)
  }

  fn is_empty(&self) -> bool {
    self.is_empty()
  }

  fn into_rope(self) -> Rope<'a> {
    self
  }

  fn get_byte(&self, byte_index: usize) -> Option<u8> {
    self.get_byte(byte_index)
  }
}

impl<'a> SourceText<'a> for &'a str {
  fn split_into_lines(&self) -> impl Iterator<Item = Self> {
    split_str(self, b'\n')
  }

  fn len(&self) -> usize {
    (*self).len()
  }

  fn ends_with(&self, value: &str) -> bool {
    (*self).ends_with(value)
  }

  fn char_indices(&self) -> impl Iterator<Item = (usize, char)> {
    (*self).char_indices()
  }

  fn byte_slice(&self, range: Range<usize>) -> Self {
    self.get(range).unwrap_or_default()
  }

  fn is_empty(&self) -> bool {
    (*self).is_empty()
  }

  fn into_rope(self) -> Rope<'a> {
    Rope::from_str(self)
  }

  fn get_byte(&self, byte_index: usize) -> Option<u8> {
    self.as_bytes().get(byte_index).copied()
  }
}
