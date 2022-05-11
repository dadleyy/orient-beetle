use serde::Deserialize;

#[derive(Deserialize, Debug)]
struct MessagePayload {
  device: String,
  message: String,
}

pub async fn send_message(mut request: tide::Request<super::worker::Worker>) -> tide::Result {
  let mut stream = request.state().redis().await?;

  let payload = request.body_json::<MessagePayload>().await.map_err(|err| {
    log::warn!("invalid payload - {err}");
    err
  })?;

  log::debug!("message payload {payload:?}");
  let find_result = kramer::execute(
    &mut stream,
    kramer::Command::Sets(kramer::SetCommand::IsMember(
      crate::constants::REGISTRAR_INDEX,
      &payload.device,
    )),
  )
  .await?;

  let found = match find_result {
    kramer::Response::Item(kramer::ResponseValue::Integer(1)) => true,
    other => {
      log::warn!("unable to find '{}' - {other:?}", payload.device);
      false
    }
  };

  if found != true {
    return Ok(tide::Response::new(404));
  }

  kramer::execute(
    &mut stream,
    kramer::Command::List(kramer::ListCommand::Push(
      (kramer::Side::Right, kramer::Insertion::Always),
      format!("ob:{}", payload.device),
      kramer::Arity::One(&payload.message),
    )),
  )
  .await?;

  Ok("".into())
}
