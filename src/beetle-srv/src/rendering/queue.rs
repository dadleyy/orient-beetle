use serde::{Deserialize, Serialize};
use std::io;

/// When adding messages that will be popped by our renderer, associate each with some kind of
/// authority so we can trace back why things appeared.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub enum QueuedRenderAuthority {
  /// The queued render was sent by the cli.
  CommandLine,

  /// The queued render was sent by the registrar.
  Registrar,

  /// The queued render was sent by a user.
  User(String),
}

/// This is the schema of our messages that will be pushed onto a rendering queue that will be
/// popped by some background worker.
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub struct QueuedRender<S> {
  /// A unique id associated with this attempt.
  pub(super) id: String,
  /// The authority.
  pub(super) auth: QueuedRenderAuthority,
  /// The content.
  pub(super) layout: super::RenderVariant<S>,
  /// The target.
  pub(super) device_id: String,
}

/// A wrapping type that will be encrypted when pushed into redis.
#[derive(Deserialize, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) struct QueuedRenderEncrypted<S> {
  /// The exp field used by jwt.
  pub(super) exp: u32,

  /// The inner job type.
  pub(super) job: QueuedRender<S>,
}

/// A type that wraps a connection and provides everything we need to add messages to our rendering
/// queue.
pub struct Queue<'a, C> {
  /// The underlying connection to redis.
  connection: &'a mut C,

  /// The secret being used to encrypt
  secret: &'a String,
}

impl<'a, C> Queue<'a, C>
where
  C: async_std::io::Read + async_std::io::Write + std::marker::Unpin,
{
  /// Creates the new rendering queue around a connection.
  pub fn new(connection: &'a mut C, secret: &'a String) -> Self {
    Queue { connection, secret }
  }

  /// Creates a queued render, serializes it, and adds it to the redis list for popping later.
  pub async fn queue<S, T>(
    &mut self,
    device_id: S,
    auth: &QueuedRenderAuthority,
    layout: super::RenderVariant<T>,
  ) -> io::Result<(String, i64)>
  where
    S: AsRef<str>,
    T: Serialize,
  {
    let id = uuid::Uuid::new_v4().to_string();
    let queued_item = QueuedRender {
      id: id.clone(),
      layout,
      device_id: device_id.as_ref().to_string(),
      auth: auth.clone(),
    };

    // TODO(job_encryption): using jwt here for ease, not the fact that it is the best. The
    // original intent in doing this was to avoid having plaintext in our redis messages.
    // Leveraging and existing depedency like `aes-gcm` would be awesome.
    let header = jsonwebtoken::Header::default();
    let secret = jsonwebtoken::EncodingKey::from_secret(self.secret.as_bytes());
    let exp = chrono::Utc::now()
      .checked_add_signed(chrono::Duration::minutes(1440))
      .unwrap_or_else(chrono::Utc::now)
      .timestamp() as u32;
    let json = jsonwebtoken::encode(&header, &QueuedRenderEncrypted { exp, job: queued_item }, &secret)
      .map_err(|error| io::Error::new(io::ErrorKind::Other, format!("unable to encrypt job - {error}")))?;

    log::info!("pushing into render '{id}' iinto rendering queue");

    let res = kramer::execute(
      &mut self.connection,
      kramer::Command::Lists(kramer::ListCommand::Push(
        (kramer::Side::Right, kramer::Insertion::Always),
        crate::constants::RENDERING_QUEUE,
        kramer::Arity::One(&json),
      )),
    )
    .await?;

    match res {
      kramer::Response::Item(kramer::ResponseValue::Integer(amount)) => {
        log::info!("rendering request queued. current queue size {amount}");
        Ok((id, amount))
      }
      other => Err(io::Error::new(
        io::ErrorKind::Other,
        format!("strange response from queue attempt - {other:?}"),
      )),
    }
  }
}
