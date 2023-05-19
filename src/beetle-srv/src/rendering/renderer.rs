use std::io;

/// The internal struct used by our entrypoint each iteration of the interval.
struct Worker {
  /// The configuration, and parsed options for mongo.
  config: (crate::registrar::Configuration, mongodb::options::ClientOptions),

  /// Connection pools.
  connections: (
    Option<mongodb::Client>,
    Option<async_tls::client::TlsStream<async_std::net::TcpStream>>,
  ),
}

impl Worker {
  /// Constructs the worker, with some validation on the configuration.
  async fn new(config: crate::registrar::Configuration) -> io::Result<Self> {
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

      let cmd = kramer::Command::<&str, &str>::Lists(kramer::ListCommand::Pop(
        kramer::Side::Left,
        crate::constants::RENDERING_QUEUE,
        None,
      ));

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

          serde_json::from_str::<super::queue::QueuedRender<String>>(payload.as_str())
            .map_err(|error| {
              log::warn!("unable to deserialize queued item - {error}");
              error
            })
            .ok()
        }
        Ok(other) => {
          log::warn!("strange response from rendering queue pop - {other:?}");
          None
        }
      };

      if let Some(queued_render) = payload {
        log::info!("found render, rasterizing + publish to '{}'", queued_render.device_id);

        let queue_id = crate::redis::device_message_queue_id(&queued_render.device_id);
        let serialized = serde_json::to_string(&queued_render)?;

        let errors = match self.send_layout(&mut c, &queue_id, queued_render.layout.clone()).await {
          Ok(_) => vec![],
          Err(error) => vec![format!("{error}")],
        };

        let mongo = self
          .connections
          .0
          .as_mut()
          .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "no mongo connection".to_string()))?;

        let devices = mongo
          .database(&self.config.0.mongo.database)
          .collection::<crate::types::DeviceDiagnostic>(&self.config.0.mongo.collections.device_diagnostics);

        if let Err(error) = devices
          .update_one(
            bson::doc! { "id": &queued_render.device_id },
            bson::doc! {
                "$inc": { "sent_message_count": 1 },
                "$push": { "sent_messages": serialized },
                "$push": { "render_failures": { "$each": errors } },
            },
            None,
          )
          .await
        {
          log::warn!("unable to update device diagnostic total message count - {error}");
        }

        log::debug!("mongo diagnostics updated for '{}'", queued_render.device_id);
      }

      self.connections.1 = Some(c);
    }

    Ok(())
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
      super::RenderVariant::Lighting(layout) => {
        let inner = match &layout {
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
      super::RenderVariant::Layout(layout) => {
        let formatted_buffer = layout.rasterize((400, 300))?;

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
}

/// The main entrypoint for our renderers.
pub async fn run(config: crate::registrar::Configuration) -> io::Result<()> {
  let mut tick = 0u8;
  let mut interval = async_std::stream::interval(std::time::Duration::from_secs(1));
  let mut worker = Worker::new(config).await?;
  log::info!("renderer running");

  loop {
    async_std::stream::StreamExt::next(&mut interval).await;

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
