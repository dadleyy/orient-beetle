use std::io;

pub enum SizeResult {
  Complete(i32),
  Incomplete(io::Result<SizeCollector>),
}

#[derive(Debug, PartialEq)]
pub struct SizeCollector {
  pub size: i32,
  pub terminating: bool,
  pub sign: i32,
}

impl Default for SizeCollector {
  fn default() -> Self {
    Self {
      size: 0,
      terminating: false,
      sign: 1,
    }
  }
}

impl SizeCollector {
  fn take(mut self, token: u8) -> SizeResult {
    if token == b'-' {
      return SizeResult::Incomplete(Ok(Self {
        size: 0,
        terminating: false,
        sign: -1,
      }));
    }

    if token == b'\r' {
      return SizeResult::Incomplete(Ok(Self {
        size: self.size,
        terminating: true,
        sign: self.sign,
      }));
    }

    if self.terminating && token == b'\n' {
      return SizeResult::Complete(self.size * self.sign);
    }

    self.size = (self.size * 10) + (token - b'0') as i32;

    SizeResult::Incomplete(Ok(self))
  }
}

#[derive(Debug, PartialEq)]
pub enum StringCollector {
  Sizing(SizeCollector),

  Collecting {
    /// The length of the string.
    total_count: i32,
    current_count: i32,
    items: Vec<u8>,
  },
}

pub enum StringCollectorResult {
  Finished(Vec<u8>),
  Incomplete(io::Result<StringCollector>),
}

impl StringCollector {
  pub fn take(self, token: u8) -> StringCollectorResult {
    match self {
      Self::Sizing(collector) => {
        if token == b'$' {
          return StringCollectorResult::Incomplete(Ok(Self::Sizing(SizeCollector::default())));
        }
        match collector.take(token) {
          SizeResult::Complete(size) => StringCollectorResult::Incomplete(Ok(Self::Collecting {
            total_count: size,
            current_count: 0,
            items: vec![],
          })),
          SizeResult::Incomplete(Ok(collector)) => StringCollectorResult::Incomplete(Ok(Self::Sizing(collector))),
          SizeResult::Incomplete(Err(e)) => StringCollectorResult::Incomplete(Err(e)),
        }
      }
      Self::Collecting {
        current_count,
        total_count,
        mut items,
      } => {
        if current_count < total_count {
          items.push(token);
        }

        if current_count == total_count + 1 {
          return StringCollectorResult::Finished(items);
        }

        StringCollectorResult::Incomplete(Ok(Self::Collecting {
          current_count: current_count + 1,
          total_count,
          items,
        }))
      }
    }
  }
}

#[derive(Debug, PartialEq)]
pub enum ArrayMessage {
  Sizing(SizeCollector),

  Collecting {
    /// The length of the array.
    total_count: i32,

    current_count: i32,

    head: StringCollector,

    /// Our list of accumulated items.
    items: Vec<Vec<u8>>,
  },
}

impl Default for ArrayMessage {
  fn default() -> Self {
    Self::Sizing(SizeCollector::default())
  }
}

pub enum ArrayCollectionResult {
  Finished(Vec<Vec<u8>>),
  Incomplete(io::Result<ArrayMessage>),
}

impl ArrayMessage {
  pub fn take(self, token: u8) -> ArrayCollectionResult {
    match self {
      Self::Sizing(collector) => match collector.take(token) {
        SizeResult::Complete(size) => {
          if size < 1 {
            return ArrayCollectionResult::Finished(vec![]);
          }
          ArrayCollectionResult::Incomplete(Ok(Self::Collecting {
            total_count: size,
            current_count: 0,
            head: StringCollector::Sizing(SizeCollector::default()),
            items: vec![],
          }))
        }
        SizeResult::Incomplete(Ok(collector)) => ArrayCollectionResult::Incomplete(Ok(Self::Sizing(collector))),
        SizeResult::Incomplete(Err(e)) => ArrayCollectionResult::Incomplete(Err(e)),
      },
      Self::Collecting {
        total_count,
        current_count,
        mut items,
        head,
      } => match head.take(token) {
        StringCollectorResult::Finished(buffer) => {
          items.push(buffer);

          if current_count + 1 == total_count {
            return ArrayCollectionResult::Finished(items);
          }

          ArrayCollectionResult::Incomplete(Ok(Self::Collecting {
            total_count,
            current_count: current_count + 1,
            items,
            head: StringCollector::Sizing(SizeCollector::default()),
          }))
        }
        StringCollectorResult::Incomplete(Ok(next)) => ArrayCollectionResult::Incomplete(Ok(Self::Collecting {
          total_count,
          current_count,
          items,
          head: next,
        })),
        StringCollectorResult::Incomplete(Err(next)) => ArrayCollectionResult::Incomplete(Err(next)),
      },
    }
  }
}

#[derive(Debug, Default)]
pub enum MessageState {
  #[default]
  Initial,

  Array(ArrayMessage),

  String(StringCollector),

  Int(SizeCollector),

  Complete(RedisResponse),

  Failed(io::Error),
}

impl MessageState {
  pub fn take(self, token: u8) -> Self {
    match (self, token) {
      (Self::Initial, b'*') => Self::Array(ArrayMessage::default()),
      (Self::Initial, b':') => Self::Int(SizeCollector::default()),
      (Self::Initial, b'-') => Self::Initial,
      (Self::Initial, b'$') => Self::String(StringCollector::Sizing(SizeCollector::default())),

      (Self::Complete(response), _) => Self::Complete(response),

      (Self::Int(collector), other) => match collector.take(other) {
        SizeResult::Incomplete(Ok(ar)) => Self::Int(ar),
        SizeResult::Incomplete(Err(e)) => Self::Failed(e),
        SizeResult::Complete(amount) => Self::Complete(RedisResponse::Int(amount)),
      },

      (Self::String(string), other) => match string.take(other) {
        StringCollectorResult::Incomplete(Ok(ar)) => Self::String(ar),
        StringCollectorResult::Incomplete(Err(e)) => Self::Failed(e),
        StringCollectorResult::Finished(buffer) => Self::Complete(RedisResponse::String(buffer)),
      },

      (Self::Array(array), other) => match array.take(other) {
        ArrayCollectionResult::Incomplete(Ok(ar)) => Self::Array(ar),
        ArrayCollectionResult::Incomplete(Err(e)) => Self::Failed(e),
        ArrayCollectionResult::Finished(buffers) => Self::Complete(RedisResponse::Array(buffers)),
      },

      (Self::Initial, _) => Self::Failed(io::Error::new(io::ErrorKind::Other, "invalid start")),
      (Self::Failed(e), _) => Self::Failed(e),
    }
  }
}

#[derive(Debug, PartialEq)]
pub enum RedisResponse {
  Empty,
  Failed(String),
  Array(Vec<Vec<u8>>),
  String(Vec<u8>),
  Int(i32),
}

impl FromIterator<u8> for RedisResponse {
  fn from_iter<I>(i: I) -> Self
  where
    I: IntoIterator<Item = u8>,
  {
    let mut state = MessageState::default();

    for tok in i.into_iter() {
      state = state.take(tok);

      if let MessageState::Complete(response) = state {
        return response;
      }

      if let MessageState::Failed(error) = state {
        return RedisResponse::Failed(error.to_string());
      }
    }

    Self::Empty
  }
}

#[cfg(test)]
mod tests {
  use super::RedisResponse;

  #[test]
  fn test_array_empty() {
    let input = "*-1\r\n";
    let result = input.as_bytes().iter().copied().collect::<RedisResponse>();
    assert_eq!(result, RedisResponse::Array(vec![]));
  }

  #[test]
  fn test_int() {
    let input = ":123\r\n";
    let result = input.as_bytes().iter().copied().collect::<RedisResponse>();
    assert_eq!(result, RedisResponse::Int(123));
  }

  #[test]
  fn test_bulk_str() {
    let input = "$2\r\nhi\r\n";
    let result = input.as_bytes().iter().copied().collect::<RedisResponse>();
    assert_eq!(
      result,
      RedisResponse::String("hi".as_bytes().iter().copied().collect::<Vec<u8>>())
    )
  }

  #[test]
  fn test_array_message_many() {
    let mut many = "*10\r\n".to_string();
    for _ in 0..10 {
      many.push_str("$2\r\nhi\r\n");
    }
    let result = many.as_bytes().iter().copied().collect::<RedisResponse>();
    assert_eq!(
      result,
      RedisResponse::Array(vec![
        b"hi".to_vec(),
        b"hi".to_vec(),
        b"hi".to_vec(),
        b"hi".to_vec(),
        b"hi".to_vec(),
        b"hi".to_vec(),
        b"hi".to_vec(),
        b"hi".to_vec(),
        b"hi".to_vec(),
        b"hi".to_vec(),
      ])
    )
  }

  #[test]
  fn test_array_message_single() {
    let response = "*1\r\n$2\r\nhi\r\n"
      .as_bytes()
      .iter()
      .copied()
      .collect::<RedisResponse>();
    assert_eq!(response, RedisResponse::Array(vec![b"hi".to_vec()]));
  }
}
