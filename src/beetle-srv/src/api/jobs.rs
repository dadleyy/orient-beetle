use serde::Deserialize;

/// The payload for looking up a device by id.
#[derive(Debug, Deserialize)]
struct LookupQuery {
  /// The id of a device in question.
  id: String,
}

/// Attempts to find a job result based on the id of the job provided in the query params.
pub async fn find(request: tide::Request<super::worker::Worker>) -> tide::Result {
  let query = request.query::<LookupQuery>().map_err(|error| {
    log::warn!("invalid job lookup - {error}");
    tide::Error::from_str(422, "missing-id")
  })?;
  let worker = request.state();
  worker
    .request_authority(&request)
    .await?
    .ok_or_else(|| {
      log::warn!("no user found");
      tide::Error::from_str(404, "missing-user")
    })
    .map_err(|error| {
      log::warn!("unable to determine request authority - {error}");
      error
    })?;

  log::info!("attempting to find result for job '{}'", query.id);

  let res = worker
    .command::<&str, &str>(&kramer::Command::Hashes(kramer::HashCommand::Get(
      crate::constants::REGISTRAR_JOB_RESULTS,
      Some(kramer::Arity::One(&query.id)),
    )))
    .await
    .map_err(|error| {
      log::warn!("unable to lookup job - {error}");
      tide::Error::from_str(500, "internal error")
    })?;

  match res {
    kramer::Response::Item(kramer::ResponseValue::String(contents)) => {
      log::info!("found job contents - '{contents:?}'");
      let parsed = serde_json::from_str::<crate::job_result::JobResult>(&contents).map_err(|error| {
        log::warn!("unable to lookup job - {error}");
        tide::Error::from_str(500, "internal error")
      })?;

      tide::Body::from_json(&parsed).map(|body| tide::Response::builder(200).body(body).build())
    }
    other => {
      log::warn!("unable to lookup job - {other:?}");
      Err(tide::Error::from_str(500, "internal error"))
    }
  }
}
