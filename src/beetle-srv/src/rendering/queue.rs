use serde::Serialize;
use std::io;

/// When adding messages that will be popped by our renderer, associate each with some kind of
/// authority so we can trace back why things appeared.
#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub enum QueuedRenderAuthority {
  /// The queued render was sent by the cli.
  CommandLine,

  /// The queued render was sent by a user.
  User(String),
}

/// This is the schema of our messages that will be pushed onto a rendering queue that will be
/// popped by some background worker.
#[derive(Serialize, Debug)]
#[serde(rename_all = "snake_case")]
pub struct QueuedRender<S> {
  /// A unique id associated with this attempt.
  id: String,
  /// The authority.
  auth: QueuedRenderAuthority,
  /// The content.
  layout: super::RenderLayout<S>,
  /// The target.
  device_id: String,
}

/// A type that wraps a connection and provides everything we need to add messages to our rendering
/// queue.
pub struct Queue<'a, C> {
  /// The underlying connection to redis.
  connection: &'a mut C,
}

impl<'a, C> Queue<'a, C>
where
  C: async_std::io::Read + async_std::io::Write + std::marker::Unpin,
{
  /// Creates the new rendering queue around a connection.
  pub fn new(connection: &'a mut C) -> Self {
    Queue { connection }
  }

  /// Creates a queued render, serializes it, and adds it to the redis list for popping later.
  pub async fn queue<S>(
    &mut self,
    device_id: S,
    auth: &QueuedRenderAuthority,
    layout: super::RenderLayout<S>,
  ) -> io::Result<(String, i64)>
  where
    S: AsRef<str> + Serialize,
  {
    let queued_item = QueuedRender {
      id: uuid::Uuid::new_v4().to_string(),
      layout,
      device_id: device_id.as_ref().to_string(),
      auth: auth.clone(),
    };
    let json = serde_json::to_string(&queued_item)?;
    log::info!("attempting to push a rendering request onto the queue for {json:?}");
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
        Ok((queued_item.id, amount))
      }
      other => Err(io::Error::new(
        io::ErrorKind::Other,
        format!("strange response from queue attempt - {other:?}"),
      )),
    }
  }
}
