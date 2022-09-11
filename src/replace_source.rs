use crate::Source;

pub struct ReplaceSource {
  inner: Box<dyn Source>,
  name: String,
  replacements: Vec<Replacement>,
}
