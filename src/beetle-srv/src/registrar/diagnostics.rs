use serde::Serialize;
use std::io;

/// This type is used by mongo when an existing record is _not_ found.
#[derive(Serialize)]
struct DeviceDiagnosticSetOnInsert {
  /// When inserting, start with the current timestamp .
  #[serde(with = "chrono::serde::ts_milliseconds")]
  first_seen: chrono::DateTime<chrono::Utc>,
}

/// If mongo already has an entry for this device, this type will be used for the "update" portion
/// of our request.
#[derive(Serialize)]
struct DeviceDiagnosticUpsert<'a> {
  /// The id of our device.
  id: &'a String,

  /// The timestamp we should now be updating.
  #[serde(with = "chrono::serde::ts_milliseconds")]
  last_seen: chrono::DateTime<chrono::Utc>,
}

/// The second main function of our registrar is to keep our server informed of the active devices
/// by pulling off a queue that is pushed to by devices during regular operating procedure. With an
/// id pulled from the queue, we will store:
///
/// 1. the current timestamp in a hash of `<id> -> timestamp`
/// 2. the id we received in a `Set` for easy indexing.
pub(super) async fn mark_active<R>(worker: &mut super::Worker, mut stream: &mut R) -> io::Result<usize>
where
  R: async_std::io::Read + async_std::io::Write + Unpin,
{
  let taken = kramer::execute(
    &mut stream,
    kramer::Command::Lists::<&str, bool>(kramer::ListCommand::Pop(
      kramer::Side::Left,
      crate::constants::REGISTRAR_INCOMING,
      None,
    )),
  )
  .await?;

  if let kramer::Response::Item(kramer::ResponseValue::String(id)) = taken {
    log::trace!("found device push from '{id}' waiting in incoming queue");

    let super::worker::WorkerMongo {
      client: mongo_client,
      config: mongo_config,
    } = &worker.mongo;
    let collection = mongo_client
      .database(&mongo_config.database)
      .collection::<crate::types::DeviceDiagnostic>(&mongo_config.collections.device_diagnostics);

    // Attempt to update the diagnostic information in mongo. We only really want to set `last_seen`
    // on every message; to set `first_seen`, we'll take advantage of mongo's `$setOnInsert`
    // operation.
    let device_diagnostic = collection
      .find_one_and_update(
        bson::doc! { "id": &id },
        bson::to_document(&DeviceDiagnosticUpsert {
          id: &id,
          last_seen: chrono::Utc::now(),
        })
        .and_then(|left| {
          bson::to_document(&DeviceDiagnosticSetOnInsert {
            first_seen: chrono::Utc::now(),
          })
          .map(|right| (left, right))
        })
        .map(|(l, r)| {
          bson::doc! {
            "$set": l,
            "$setOnInsert": r
          }
        })
        .map_err(|error| {
          log::warn!("unable to build upsert doc - {error}");
          io::Error::new(io::ErrorKind::Other, format!("{error}"))
        })?,
        Some(
          mongodb::options::FindOneAndUpdateOptions::builder()
            .upsert(true)
            .return_document(mongodb::options::ReturnDocument::After)
            .build(),
        ),
      )
      .await
      .map_err(|error| {
        log::warn!("unable to upsert device diagnostic - {error}");
        io::Error::new(io::ErrorKind::Other, format!("{error}"))
      })?
      .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "upsert failed"))?;

    match &device_diagnostic.registration_state {
      Some(crate::types::DeviceDiagnosticRegistration::Initial) | None => {
        log::info!(
          "first message received by device '{}'! sending initial qr code",
          device_diagnostic.id
        );

        let mut queue = crate::rendering::queue::Queue::new(stream);
        let mut initial_url = http_types::Url::parse(&worker.config.initial_scannable_addr).map_err(|error| {
          log::warn!("unable to create initial url for device - {error}");
          io::Error::new(io::ErrorKind::Other, format!("{error}"))
        })?;

        // scope our mutable borrow/mutation so it is dropped before we take ownship when we
        // `to_string` it onto our layout.
        {
          let mut query = initial_url.query_pairs_mut();
          query.append_pair("device_target_id", &device_diagnostic.id);
        }

        let layout = crate::rendering::RenderVariant::scannable(initial_url.to_string());
        if let Err(error) = queue
          .queue(
            &device_diagnostic.id,
            &crate::rendering::QueuedRenderAuthority::Registrar,
            layout,
          )
          .await
        {
          log::warn!("unable to queue welcome message to device - {error}");
        } else {
          log::info!(
            "first render queued, updating diagnostic registration state for '{}'",
            device_diagnostic.id
          );
          let updated_reg = crate::types::DeviceDiagnosticRegistration::PendingRegistration;
          let serialized_registration = bson::to_bson(&updated_reg).map_err(|error| {
            log::warn!("unable to serialize registration_state: {error}");
            io::Error::new(io::ErrorKind::Other, format!("{error}"))
          })?;

          if let Err(error) = collection
            .find_one_and_update(
              bson::doc! { "id": &device_diagnostic.id },
              bson::doc! { "$set": { "registration_state": serialized_registration } },
              mongodb::options::FindOneAndUpdateOptions::builder()
                .upsert(true)
                .return_document(mongodb::options::ReturnDocument::After)
                .build(),
            )
            .await
          {
            log::warn!("unable to update device registration state - {error}");
          }
        }
      }
      Some(other) => {
        log::trace!("device '{}' has {other:?} previous registration", device_diagnostic.id);
      }
    }

    // Store the device identity in a set; this will allow us to iterate over the list of
    // active ids more easily later.
    let setter = kramer::Command::Sets(kramer::SetCommand::Add(
      crate::constants::REGISTRAR_INDEX,
      kramer::Arity::One(id.as_str()),
    ));
    kramer::execute(&mut stream, setter).await?;

    log::info!("updated device '{}' diagnostics", device_diagnostic.id);

    return Ok(1usize);
  }

  Ok(0usize)
}
