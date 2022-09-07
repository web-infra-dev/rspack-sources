use std::cmp::{max, min};
use std::rc::Rc;
use std::slice::SliceIndex;

use smol_str::SmolStr;
use sourcemap::{SourceMapBuilder, Token};

use crate::source::Source;

pub struct ReplaceSource<T: Source> {
  inner: T,
  replacements: Vec<Replacement>,
}

struct Replacement {
  start: i32,
  end: i32,
  content: SmolStr,
  name: Option<SmolStr>,
}

struct SourceCodeIndexer {
  code: SmolStr,
  lines_lens_pre_sum: Vec<u32>,
}

enum PaddedToken<'a> {
  Token(Token<'a>),
  Line(u32),
}

impl PaddedToken<'_> {
  fn from_token(token: Token) -> PaddedToken {
    PaddedToken::Token(token)
  }

  fn from_line(line: u32) -> PaddedToken<'static> {
    PaddedToken::Line(line)
  }

  fn get_dst_line(&self) -> u32 {
    match self {
      PaddedToken::Token(token) => token.get_dst_line(),
      PaddedToken::Line(line) => *line,
    }
  }

  fn get_src_line(&self) -> Option<u32> {
    match self {
      PaddedToken::Token(token) => Some(token.get_src_line()),
      PaddedToken::Line(_) => None,
    }
  }

  fn get_src_col(&self) -> Option<u32> {
    match self {
      PaddedToken::Token(token) => Some(token.get_src_col()),
      PaddedToken::Line(_) => None,
    }
  }

  fn get_src_id(&self) -> Option<u32> {
    match self {
      PaddedToken::Token(token) => Some(token.get_src_id()),
      PaddedToken::Line(_) => None,
    }
  }

  fn get_name(&self) -> Option<&str> {
    match self {
      PaddedToken::Token(token) => token.get_name(),
      PaddedToken::Line(_) => None,
    }
  }
}

impl SourceCodeIndexer {
  fn new(code: SmolStr) -> Self {
    // use a prefix sum array for finding position by (line, col)
    // line_lens_pre_sum[i] = sum of lines_lens[0..i-1]
    let every_line_lens: Vec<u32> = code.split('\n').map(|line| line.len() as u32).collect();
    let lines_lens_pre_sum: Vec<u32> = [0]
      .iter()
      .chain(every_line_lens.iter())
      .scan(0, |sum, i| {
        *sum += i;
        Some(*sum)
      })
      .collect();
    Self {
      code,
      lines_lens_pre_sum,
    }
  }

  fn get<I>(&self, idx: I) -> Option<&I::Output>
  where
    I: SliceIndex<str>,
  {
    self.code.get(idx)
  }

  fn max_position(&self) -> u32 {
    self.code.len() as u32
  }

  fn get_position_by_line_col(&self, line: &u32, col: &u32) -> Option<u32> {
    // sum of char before this line + '\n' * line + col
    self
      .lines_lens_pre_sum
      .get(*line as usize)
      .map(|chars| chars + line + col)
  }

  fn get_position_by_token(&self, token: &Token) -> Option<u32> {
    self.get_position_by_line_col(&token.get_dst_line(), &token.get_dst_col())
  }
}

impl<T: Source> ReplaceSource<T> {
  pub fn new(source: T) -> Self {
    Self {
      inner: source,
      replacements: vec![],
    }
  }

  pub fn original(&self) -> &T {
    &self.inner
  }

  pub fn insert(&mut self, start: i32, content: &str, name: Option<&str>) {
    self.replacements.push(Replacement {
      start,
      end: start,
      content: content.into(),
      name: name.map(|s| s.into()),
    });
  }

  pub fn replace(&mut self, start: i32, end: i32, content: &str, name: Option<&str>) {
    self.replacements.push(Replacement {
      start,
      end,
      content: content.into(),
      name: name.map(|s| s.into()),
    });
  }

  fn sort_replacement(&mut self) {
    self
      .replacements
      .sort_by(|a, b| (a.start, a.end).cmp(&(b.start, b.end)));
  }
}

impl<T: Source> Source for ReplaceSource<T> {
  fn map(&mut self, option: &crate::GenMapOption) -> Option<Rc<sourcemap::SourceMap>> {
    self.inner.map(option).map(|inner_source_map| {
      self.sort_replacement();

      // source map may be ";;;AAAA", which means some target code line may not have (src_line, src_col),
      // but replacement may be delete these lines.
      // To simplify the code, below code will add PaddedToken for these lines.
      let mut tokens: Vec<PaddedToken> = vec![];
      let mut line = 0;
      inner_source_map.tokens().for_each(|token| {
        while token.get_dst_line() > line {
          tokens.push(PaddedToken::from_line(line));
          line += 1;
        }
        tokens.push(PaddedToken::from_token(token));
        line = token.get_dst_line() + 1;
      });

      let source_code_indexer = SourceCodeIndexer::new(self.inner.source());
      let token_positions: Vec<u32> = tokens
        .iter()
        .map(|padded_token| match padded_token {
          PaddedToken::Token(token) => source_code_indexer.get_position_by_token(token).unwrap(),
          PaddedToken::Line(line) => source_code_indexer
            .get_position_by_line_col(line, &0)
            .unwrap(),
        })
        .chain([source_code_indexer.max_position()].into_iter())
        .collect();

      // check if source_content[line][col] is equal to expect
      // Why this is needed?
      //
      // For example, there is an source_map like (It's OriginalSource)
      //    source_code: "jsx || tsx"
      //    mappings:    ↑
      //    target_code: "jsx || tsx"
      // If replace || to &&, there will be some new mapping information
      //    source_code: "jsx || tsx"
      //    mappings:    ↑    ↑  ↑
      //    target_code: "jsx && tsx"
      //
      // In this case, because source_content[line][col] is equal to target, we can split this mapping correctly,
      // Therefore, we can add some extra mappings for this replace operation.
      //
      // But for this example, source_content[line][col] is not equal to target (It's SourceMapSource)
      //    source_code: "<div />"
      //    mappings:    ↑
      //    target_code: "jsx || tsx"
      // If replace || to && also, then
      //    source_code: "<div />"
      //    mappings:    ↑
      //    target_code: "jsx && tsx"
      //
      // In this case, we can't split this mapping.
      // webpack-sources also have this function, refer https://github.com/webpack/webpack-sources/blob/main/lib/ReplaceSource.js#L158
      let check_origin_content = |source_content: Option<&str>,
                                  line: Option<u32>,
                                  col: Option<u32>,
                                  expect: Option<&str>| {
        if line.is_none() || col.is_none() || expect.is_none() {
          return false;
        }
        let content = source_content.and_then(|s| s.split('\n').nth(line.unwrap() as usize));
        if content.is_none() {
          return false;
        }
        return content
          .unwrap()
          .get(col.unwrap() as usize..col.unwrap() as usize + expect.unwrap().len())
          == expect;
      };

      let mut sm_builder = SourceMapBuilder::new(inner_source_map.get_file());
      let mut generated_line = 0;
      let mut generated_column = 0;
      let mut last_generated_line = None;
      let mut last_src_line = None;
      let mut last_src_column = None;
      let mut last_src_id = None;

      let mut add_mapping = |content_length: u32,
                             new_line_num: u32,
                             src_line: Option<u32>,
                             src_col: Option<u32>,
                             src_id: Option<u32>,
                             name: Option<&str>| {
        // Only add token when content_length is not empty or it is "\n" (which means content_length is 0 but new_line_num is 1)
        // This can avoid source_map have duplicate (dst_line, dst_col) token.
        // Because there may be an operation like inserting an empty string.
        if new_line_num == 0 && content_length == 0 {
          return;
        }

        if option.columns || generated_column == 0 {
          if let Some(src_line_v) = src_line {
            if let Some(src_col_v) = src_col {
              // when there are two position corresponding one src position, should not emit twice
              // for example: 1:0 -> 1:1, 1:5 -> 1:1 x
              //              1:0 -> 1:1 ✓
              // but if two position are not in one line is ok
              // for example:
              //              1:0 -> 1:1, 2:0 -> 1:1 ✓
              if src_line != last_src_line
                || src_col != last_src_column
                || src_id != last_src_id
                || Some(generated_line) != last_generated_line
              {
                sm_builder.add(
                  generated_line,
                  generated_column,
                  src_line_v,
                  src_col_v,
                  src_id.and_then(|idx| inner_source_map.get_source(idx)),
                  name,
                );
                last_generated_line = Some(generated_line);
                last_src_line = src_line;
                last_src_column = src_col;
                last_src_id = src_id;
              }
            }
          }
        }

        if new_line_num != 0 {
          generated_line += new_line_num;
          generated_column = 0;
        } else {
          generated_column += content_length;
        }
      };

      let mut replace_idx = 0;
      let mut token_idx = 0;
      let mut token_split_offset = 0;
      let mut src_offset = 0;

      while replace_idx < self.replacements.len() && token_idx < tokens.len() {
        let replacement = &self.replacements[replace_idx];
        let token = tokens.get(token_idx).unwrap();
        let next_token = tokens.get(token_idx + 1);

        // replacement.start or end can be negative, but it just used for sort order.
        // when used, it can be regarded as 0
        let replace_start: u32 = replacement.start.try_into().unwrap_or(0);
        let replace_end: u32 = replacement.end.try_into().unwrap_or(0);
        let mut token_start = token_positions[token_idx as usize] + token_split_offset;
        let mut token_end = token_positions[token_idx as usize + 1];

        // source_code: _______________________________________________________
        //                  ↑         ↑          ↑          ↑           ↑
        //                  |         |   replace_start     |     replace_end
        //       token_original_start |                 token_end
        //                            |
        //                       token_start
        //                   __________
        //                   ↑        ↑
        //                 token_split_offset

        if replace_start >= token_end {
          // emit whole token
          add_mapping(
            token_end - token_start,
            next_token.map_or(0, |next| next.get_dst_line() - token.get_dst_line()),
            token.get_src_line(),
            token.get_src_col().map(|col| col + src_offset),
            token.get_src_id(),
            token.get_name(),
          );
          token_idx += 1;
          token_split_offset = 0;
          src_offset = 0;
        } else if replace_start > token_start {
          // emit some token and split it
          add_mapping(
            replace_start - token_start,
            0,
            token.get_src_line(),
            token.get_src_col().map(|col| col + src_offset),
            token.get_src_id(),
            token.get_name(),
          );
          if check_origin_content(
            token
              .get_src_id()
              .and_then(|idx| inner_source_map.get_source_contents(idx)),
            token.get_src_line(),
            token.get_src_col().map(|col| col + token_split_offset),
            source_code_indexer.get(token_start as usize..replace_start as usize),
          ) {
            src_offset += replace_start - token_start;
          }
          token_split_offset += replace_start - token_start;
        } else {
          // emit replacement
          let lines: Vec<&str> = replacement.content.split('\n').collect();
          lines.iter().enumerate().for_each(|(idx, line)| {
            add_mapping(
              line.len() as u32,
              if idx != lines.len() - 1 { 1 } else { 0 },
              token.get_src_line(),
              token.get_src_col().map(|col| col + src_offset),
              token.get_src_id(),
              replacement.name.as_ref().map(|name| name.as_str()),
            );
          });
          // skip any token that have been wholely replaced
          while token_end <= replace_end {
            token_idx += 1;
            token_split_offset = 0;
            src_offset = 0;
            if token_idx >= tokens.len() {
              break;
            }
            token_start = token_positions[token_idx as usize];
            token_end = token_positions[token_idx as usize + 1];
          }
          // skip part of token that have been replaced
          if token_idx < tokens.len() && replace_end > token_start {
            let token = tokens.get(token_idx).unwrap();
            if check_origin_content(
              token
                .get_src_id()
                .and_then(|idx| inner_source_map.get_source_contents(idx)),
              token.get_src_line(),
              token.get_src_col().map(|col| col + token_split_offset),
              source_code_indexer.get(token_start as usize..replace_end as usize),
            ) {
              src_offset += replace_end - token_start;
            }
            token_split_offset += replace_end - token_start;
          }
          replace_idx += 1;
        }
      }

      // just emit remaining token!
      // if token iterate finished, remaining replacement will only produce an insert on the code end, which won't produce source map
      while token_idx < tokens.len() {
        let token = tokens.get(token_idx).unwrap();
        let next_token = tokens.get(token_idx + 1);
        let token_start = token_positions[token_idx as usize] + token_split_offset;
        let token_end = token_positions[token_idx as usize + 1];

        add_mapping(
          token_end - token_start,
          next_token.map_or(0, |next| next.get_dst_line() - token.get_dst_line()),
          token.get_src_line(),
          token.get_src_col().map(|col| col + src_offset),
          token.get_src_id(),
          token.get_name(),
        );
        token_idx += 1;
        token_split_offset = 0;
        src_offset = 0;
      }

      if option.include_source_contents {
        inner_source_map
          .source_contents()
          .enumerate()
          .for_each(|(src_id, source_content)| {
            sm_builder.set_source_contents(src_id as u32, source_content);
          });
      }

      Rc::new(sm_builder.into_sourcemap())
    })
  }

  fn source(&mut self) -> SmolStr {
    self.sort_replacement();

    let inner_source_code = self.inner.source();

    // mut_string_push_str is faster that vec join
    // concatenate strings benchmark, see https://github.com/hoodie/concatenation_benchmarks-rs
    let mut source_code = String::new();
    let mut inner_pos = 0;
    for replacement in &self.replacements {
      if inner_pos < replacement.start {
        let end_pos = min(replacement.start as usize, inner_source_code.len());
        source_code.push_str(&inner_source_code[inner_pos as usize..end_pos as usize]);
      }
      source_code.push_str(&replacement.content);
      inner_pos = min(
        max(inner_pos, replacement.end),
        inner_source_code.len() as i32,
      );
    }
    source_code.push_str(&inner_source_code[inner_pos as usize..]);

    source_code.into()
  }
}
