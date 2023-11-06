use super::arguments::CommandLineArguments;
use std::io;

pub async fn get_device_id(
  args: &CommandLineArguments,
  config: &beetle::registrar::Configuration,
  mut connection: &mut beetle::redis::RedisConnection,
) -> io::Result<String> {
  let mut id_storage_path = std::path::PathBuf::from(&args.storage);
  id_storage_path.push(".device_id");

  let (id_user, id_password) = config
    .registrar
    .id_consumer_username
    .as_ref()
    .zip(config.registrar.id_consumer_password.as_ref())
    .ok_or_else(|| {
      io::Error::new(
        io::ErrorKind::Other,
        "Configuration is missing registrar burn-in credentials",
      )
    })?;

  match async_std::fs::metadata(&id_storage_path).await {
    Err(error) if error.kind() == io::ErrorKind::NotFound => {
      let burnin_auth_response = match kramer::execute(
        &mut connection,
        kramer::Command::<&str, &str>::Auth(kramer::AuthCredentials::User((id_user.as_str(), id_password.as_str()))),
      )
      .await
      {
        Ok(kramer::Response::Item(kramer::ResponseValue::String(inner))) if inner == "OK" => inner,
        other => {
          return Err(io::Error::new(
            io::ErrorKind::Other,
            format!("unable to authenticate with redis - {other:?} (as {id_user})"),
          ))
        }
      };

      log::info!("initial handshake completed {burnin_auth_response:?}, taking a device id");

      let mock_device_id = match kramer::execute(
        &mut connection,
        kramer::Command::<&str, &str>::Lists(kramer::ListCommand::Pop(
          kramer::Side::Left,
          beetle::constants::REGISTRAR_AVAILABLE,
          None,
        )),
      )
      .await
      {
        Ok(kramer::Response::Item(kramer::ResponseValue::String(id))) => id,
        other => {
          return Err(io::Error::new(
            io::ErrorKind::Other,
            format!("unable to pull id - {other:?}"),
          ))
        }
      };

      log::info!("device id taken - {mock_device_id:?}");

      match kramer::execute(
        &mut connection,
        kramer::Command::<&str, &str>::Auth(kramer::AuthCredentials::User((&mock_device_id, &mock_device_id))),
      )
      .await
      {
        Ok(kramer::Response::Item(kramer::ResponseValue::String(inner))) if inner == "OK" => (),
        other => {
          return Err(io::Error::new(
            io::ErrorKind::Other,
            format!("unable to authenticate with redis - {other:?}"),
          ))
        }
      }

      log::info!("preparing '{}' for device id storage", args.storage);
      async_std::fs::create_dir_all(&args.storage).await?;
      async_std::fs::write(&id_storage_path, &mock_device_id).await?;

      Ok(mock_device_id)
    }

    Ok(meta) if meta.is_file() => {
      log::info!("found existing device id at '{:?}'", id_storage_path);
      let loaded_id = async_std::fs::read_to_string(&id_storage_path).await?;
      log::info!("loaded device id - '{loaded_id}'");

      Ok(loaded_id)
    }
    other @ Ok(_) | other @ Err(_) => Err(io::Error::new(
      io::ErrorKind::Other,
      format!("unable to handle device id storage lookup - {other:?}"),
    )),
  }
}
