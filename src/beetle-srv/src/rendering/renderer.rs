use super::queue;
use crate::{registrar, schema};
use std::io;

/// The internal struct used by our entrypoint each iteration of the interval.
struct Worker {
  /// The configuration, and parsed options for mongo.
  config: (registrar::Configuration, mongodb::options::ClientOptions),

  /// Connection pools.
  connections: (Option<mongodb::Client>, Option<crate::redis::RedisConnection>),
}

impl Worker {
  /// Constructs the worker, with some validation on the configuration.
  async fn new(config: registrar::Configuration) -> io::Result<Self> {
    let mut connections = (None, None);

    let mongo_options = mongodb::options::ClientOptions::parse(&config.mongo.url)
      .await
      .map_err(|error| io::Error::new(io::ErrorKind::Other, format!("failed mongodb connection - {error}")))?;

    connections.0 = mongodb::Client::with_options(mongo_options.clone())
      .map_err(|error| io::Error::new(io::ErrorKind::Other, format!("failed mongodb connection - {error}")))
      .ok();

    Ok(Self {
      config: (config, mongo_options),
      connections,
    })
  }

  /// Each "working" cycle of our renderer.
  async fn tick(&mut self) -> io::Result<()> {
    // Start with an attempt to re-connect to redis.
    self.connections.1 = match self.connections.1.take() {
      None => {
        let new_connection = crate::redis::connect(&self.config.0.redis)
          .await
          .map_err(|error| {
            log::warn!("unable to connect to redis - {error}");
            error
          })
          .ok();
        log::info!("redis connection established successfully");

        new_connection
      }
      Some(connection) => Some(connection),
    };

    if let Some(mut c) = self.connections.1.take() {
      log::debug!("popping latest queued items");

      // Attempt to pop a rendering request off our queue, waiting a maximum amount of time. This
      // should be moved into configuration.
      let cmd = kramer::Command::<&str, &str>::Lists(kramer::ListCommand::Pop(
        kramer::Side::Left,
        crate::constants::RENDERING_QUEUE,
        Some((None, 5)),
      ));

      // TODO(job_encryption): using jwt here for ease, not the fact that it is the best. The
      // original intent in doing this was to avoid having plaintext in our redis messages.
      // Leveraging and existing depedency like `aes-gcm` would be awesome.
      let key = jsonwebtoken::DecodingKey::from_secret(self.config.0.registrar.vendor_api_secret.as_bytes());
      let validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);

      let payload = match kramer::execute(&mut c, cmd).await {
        Err(error) => {
          log::warn!("nuking redis connection; failed pop execution - {error}");
          return Err(error);
        }
        Ok(kramer::Response::Item(kramer::ResponseValue::Empty)) => {
          log::debug!("no messages found in queue");
          None
        }
        Ok(kramer::Response::Item(kramer::ResponseValue::String(payload))) => {
          log::debug!("found payload - '{payload}'");

          Some(payload)
        }
        Ok(kramer::Response::Array(contents)) => match contents.get(1) {
          Some(kramer::ResponseValue::String(payload)) => Some(payload.clone()),
          other => {
            log::warn!("strange response from rendering queue pop - {other:?}");
            None
          }
        },
        Ok(other) => {
          log::warn!("strange response from rendering queue pop - {other:?}");
          None
        }
      }
      .and_then(|response_string| {
        jsonwebtoken::decode::<queue::QueuedRenderEncrypted<String>>(&response_string, &key, &validation)
          .map_err(|error| {
            log::error!("registrar worker unable to decode token - {}", error);
            io::Error::new(io::ErrorKind::Other, "bad-jwt")
          })
          .ok()
      });

      if let Some(queued_render) = payload.map(|p| p.claims.job) {
        log::info!(
          "found render '{}', rasterizing + publish to '{}'",
          queued_render.id,
          queued_render.device_id
        );

        let queue_id = crate::redis::device_message_queue_id(&queued_render.device_id);

        if let Err(error) = self.clear_pending(&mut c, &queue_id).await {
          log::error!("unable to clear stale renders for '{queue_id}' - {error:?}");
        }

        // Actually attempt to rasterize the layout into bytes and send it along to the device via
        // the device redis queue.
        let queue_error = match self.send_layout(&mut c, &queue_id, queued_render.layout.clone()).await {
          Ok(_) => None,
          Err(error) => Some(format!("{error:?}")),
        };

        let histories = self.histories_collection()?;

        let message_doc = bson::to_bson(&queued_render).map_err(|error| {
          log::warn!("unable to encode message as bson! - {error}");
          io::Error::new(io::ErrorKind::Other, "serialization error".to_string())
        })?;

        match histories
          .find_one_and_update(
            bson::doc! { "device_id": &queued_render.device_id },
            bson::doc! { "$push": { "render_history": { "$each": [ ], "$slice": -10 } } },
            mongodb::options::FindOneAndUpdateOptions::builder()
              .upsert(true)
              .return_document(mongodb::options::ReturnDocument::After)
              .build(),
          )
          .await
        {
          Err(error) => {
            log::warn!(
              "render[{}] unable to truncate device '{}' history - {error}",
              queued_render.id,
              queued_render.device_id
            );
          }
          Ok(_) => {
            log::warn!(
              "render[{}] truncated history of device '{}' history successfully",
              queued_render.id,
              queued_render.device_id
            );
          }
        }

        if let Err(error) = histories
          .find_one_and_update(
            bson::doc! { "device_id": &queued_render.device_id },
            bson::doc! { "$push": { "render_history": message_doc } },
            mongodb::options::FindOneAndUpdateOptions::builder()
              .upsert(true)
              .return_document(mongodb::options::ReturnDocument::After)
              .build(),
          )
          .await
        {
          log::warn!(
            "render[{}] unable to update device '{}' message history - {error}",
            queued_render.id,
            queued_render.device_id
          );
        }

        // Lastly, update our job results hash with an entry for this render attempt. This is how
        // clients know the render has been processed in the background.
        let serialized_result = serde_json::to_string(
          &queue_error
            .map(schema::jobs::JobResult::Failure)
            .unwrap_or_else(|| schema::jobs::JobResult::Success(schema::jobs::SuccessfulJobResult::Terminal)),
        )
        .map_err(|error| {
          log::warn!("unable to complete serialization of render result - {error}");
          io::Error::new(io::ErrorKind::Other, "result-failure")
        })?;

        log::info!(
          "render[{}] setting job result for device '{}' - '{serialized_result}'",
          queued_render.id,
          queued_render.device_id
        );

        if let Err(error) = kramer::execute(
          &mut c,
          kramer::Command::Hashes(kramer::HashCommand::Set(
            crate::constants::REGISTRAR_JOB_RESULTS,
            kramer::Arity::One((&queued_render.id, serialized_result)),
            kramer::Insertion::Always,
          )),
        )
        .await
        {
          log::warn!("unable to update job result - {error}");
        }

        log::info!("job '{}' for '{}' complete", queued_render.id, queued_render.device_id);
      }

      self.connections.1 = Some(c);
    }

    Ok(())
  }

  /// Returns a handle to device history collection.
  fn histories_collection(&mut self) -> io::Result<mongodb::Collection<schema::DeviceHistoryRecord>> {
    let mongo = self
      .connections
      .0
      .as_mut()
      .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "no mongo connection".to_string()))?;

    Ok(
      mongo
        .database(&self.config.0.mongo.database)
        .collection::<schema::DeviceHistoryRecord>(&self.config.0.mongo.collections.device_histories),
    )
  }

  /// While the `tick` method is responsible for dealing with redis connections _and_ checking for
  /// a new layout, this function is solely responsible for dealing with the process of queuing
  /// that new layout onto the device queue.
  async fn send_layout<S>(
    &mut self,
    connection: &mut crate::redis::RedisConnection,
    queue_id: &str,
    layout: super::RenderVariant<S>,
  ) -> io::Result<()>
  where
    S: std::convert::AsRef<str>,
  {
    match layout {
      super::RenderVariant::Lighting(layout_container) => {
        let inner = match &layout_container.layout {
          super::LightingLayout::On => "on",
          super::LightingLayout::Off => "off",
        };
        let command = kramer::Command::Lists(kramer::ListCommand::Push(
          (kramer::Side::Left, kramer::Insertion::Always),
          queue_id,
          kramer::Arity::One(format!("{}:{inner}", crate::constants::LIGHTING_PREFIX)),
        ));
        let res = kramer::execute(connection, &command).await?;
        log::info!("pushed lighting command onto queue - '{res:?}'");
      }
      super::RenderVariant::Layout(layout_container) => {
        let formatted_buffer = layout_container.layout.rasterize((400, 300))?;

        let mut command = kramer::Command::Lists(kramer::ListCommand::Push(
          (kramer::Side::Left, kramer::Insertion::Always),
          queue_id,
          kramer::Arity::One(formatted_buffer.as_slice().iter().enumerate()),
        ));

        let res = command.execute(connection).await?;
        log::info!("pushed layout command onto queue - '{res:?}'");
      }
    }

    Ok(())
  }

  /// Given a queue id, the goal of this method is to remove all things in it. This does check the
  /// length before doing so, which is nice for logging purposes.
  async fn clear_pending(
    &mut self,
    mut connection: &mut crate::redis::RedisConnection,
    queue_id: &str,
  ) -> io::Result<()> {
    log::info!("clearing all pending renders for '{queue_id}'");
    let len = kramer::Command::<&str, &str>::Lists(kramer::ListCommand::Len(queue_id));
    let res = kramer::execute(&mut connection, &len).await?;
    let count = match res {
      kramer::Response::Item(kramer::ResponseValue::Integer(i)) => i,
      other => {
        return Err(io::Error::new(
          io::ErrorKind::Other,
          format!("invalid len response of render queue '{queue_id}'-  {other:?}"),
        ))
      }
    };

    if count <= 0 {
      log::info!("queue '{queue_id} had {count} stale messages, ignoring");
      return Ok(());
    }

    log::info!("queue '{queue_id}' has {count} stale messages, deleting");
    let del = kramer::Command::<&str, &str>::Lists(kramer::ListCommand::Trim(queue_id, count, 0));

    kramer::execute(connection, &del).await.map(|_| ()).map_err(|error| {
      io::Error::new(
        io::ErrorKind::Other,
        format!("failed deletion of stale messages on '{queue_id}' - {error:?}"),
      )
    })
  }
}

/// The main entrypoint for our renderers.
pub async fn run(config: crate::registrar::Configuration) -> io::Result<()> {
  let mut tick = 0u8;
  let mut worker = Worker::new(config).await?;
  log::info!("renderer running");

  loop {
    if let Err(error) = worker.tick().await {
      log::warn!("worker failed on tick {tick} - {error}");
    }

    if tick == 255 {
      log::info!("tick reset");
      tick = 0;
    } else {
      tick += 1;
    }
  }
}
