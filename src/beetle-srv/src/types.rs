use serde::{Deserialize, Serialize};

/// TODO: this is currently a dumping ground of non-interesting struct definitions
/// for things sent over the wire or persisted in mongo.

#[derive(Deserialize, Serialize, Debug)]
pub struct User {
  pub(crate) oid: String,
}
