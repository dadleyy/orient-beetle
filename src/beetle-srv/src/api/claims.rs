use serde::{Deserialize, Serialize};
use std::io::{Error, ErrorKind, Result};

/// The JSON structure of our session tokens.
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
  /// The time this token expires.
  pub exp: usize,
  /// The oauth user id.
  pub oid: String,
}

impl Claims {
  /// Attempts to decode the contents of the cookie into a claim/token.
  pub fn decode<T>(target: &T, secret: &str) -> Result<Self>
  where
    T: std::fmt::Display + ?Sized,
  {
    let token = format!("{}", target);
    let key = jsonwebtoken::DecodingKey::from_secret(secret.as_bytes());
    let validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);
    jsonwebtoken::decode::<Self>(token.as_str(), &key, &validation)
      .map_err(|error| {
        log::warn!("unable to decode token - {}", error);
        Error::new(ErrorKind::Other, "bad-jwt")
      })
      .map(|data| data.claims)
  }

  /// Builds a token payload for a given user id.
  pub fn for_user<T>(oid: T) -> Self
  where
    T: std::fmt::Display,
  {
    let day = chrono::Utc::now()
      .checked_add_signed(chrono::Duration::minutes(1440))
      .unwrap_or_else(chrono::Utc::now);

    let exp = day.timestamp() as usize;
    log::debug!("encoding new jwt, expires {}", exp);

    Self {
      exp,
      oid: format!("{}", oid),
    }
  }

  /// Encodes the claims as a string (serialized jwt).
  pub fn encode(&self, secret: &str) -> Result<String> {
    let header = &jsonwebtoken::Header::default();
    let secret = jsonwebtoken::EncodingKey::from_secret(secret.as_bytes());

    jsonwebtoken::encode(header, &self, &secret).map_err(|error| {
      log::warn!("unable to encode token - {}", error);
      Error::new(ErrorKind::Other, "bad-jwt")
    })
  }
}
