use serde::Deserialize;

/// Just having fun here with traits; actually figuring out where code lives relative
/// to the `bin` members and this code is not clear at the time being.
pub trait Connector: Clone + Send + Sync {
  type Value: AsRef<str> + Sync + Send;

  fn redis<'a>(&'a self) -> (&'a Self::Value, &'a Self::Value, &'a Self::Value);
}

#[derive(Deserialize, Debug)]
struct MessagePayload {
  device: String,
  message: String,
}

async fn send_message<T>(mut request: tide::Request<T>) -> tide::Result
where
  T: Connector,
{
  let (host, port, auth) = request.state().redis();
  let mut stream = crate::connect(host, port, auth).await?;

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

async fn missing<T>(_request: tide::Request<T>) -> tide::Result
where
  T: Connector,
{
  log::debug!("not-found");
  Ok("".into())
}

pub fn new<T>(connector: T) -> tide::Server<T>
where
  T: Connector + 'static,
{
  let mut app = tide::with_state::<T>(connector);

  app.at("/send-device-message").post(send_message);

  app.at("/*").all(missing);
  app.at("/").all(missing);

  app
}