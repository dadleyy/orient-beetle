use std::io;

/// The main thing our worker will be responsible for is to count the amount of available ids
/// in our pool that devices will pull down to identify themselves. If that amount reaches a
/// quantity below a specific threshold, fill it back up.
pub(super) async fn fill_pool(mut stream: &mut crate::redis::RedisConnection, min: u8) -> io::Result<usize> {
  let output = kramer::execute(
    &mut stream,
    kramer::Command::Lists::<&str, bool>(kramer::ListCommand::Len(crate::constants::REGISTRAR_AVAILABLE)),
  )
  .await?;

  let should_send = match output {
    kramer::Response::Item(kramer::ResponseValue::Integer(amount)) if amount < min as i64 => {
      log::info!("found {amount} ids available in pool, minimum amount {min}.");
      true
    }
    kramer::Response::Item(kramer::ResponseValue::Integer(amount)) => {
      log::info!("nothing to do, plenty of ids ('{amount}' vs min of '{min}')");
      false
    }
    other => {
      log::warn!("unexpected response from count: {:?}", other);
      false
    }
  };

  if !should_send {
    return Ok(0);
  }

  let ids = (0..min).map(|_| crate::identity::create()).collect::<Vec<String>>();
  let count = ids.len();

  log::info!("creating acl entries for ids {ids:?}");

  for id in &ids {
    let setuser = kramer::acl::SetUser {
      name: id.clone(),
      password: Some(id.clone()),
      commands: Some(vec!["lpop".to_string(), "blpop".to_string()]),
      keys: Some(crate::redis::device_message_queue_id(id)),
    };

    let command = kramer::Command::Acl::<String, &str>(kramer::acl::AclCommand::SetUser(setuser));

    if let Err(error) = kramer::execute(&mut stream, &command).await {
      log::warn!("unable to add acl for id '{}' - {error}", id);
    }

    let setuser = kramer::acl::SetUser {
      name: id.clone(),
      password: Some(id.clone()),
      commands: Some(vec!["rpush".to_string()]),
      keys: Some(crate::constants::REGISTRAR_INCOMING.to_string()),
    };
    let command = kramer::Command::Acl::<String, &str>(kramer::acl::AclCommand::SetUser(setuser));

    if let Err(error) = kramer::execute(&mut stream, &command).await {
      log::warn!("unable to add acl for id '{}' - {error}", id);
    }
  }

  log::info!("acl entries for new ids {ids:?} ready, pushing into registration queue",);

  let insertion = kramer::execute(
    &mut stream,
    kramer::Command::Lists(kramer::ListCommand::Push(
      (kramer::Side::Left, kramer::Insertion::Always),
      crate::constants::REGISTRAR_AVAILABLE,
      kramer::Arity::Many(ids),
    )),
  )
  .await?;

  log::debug!("insertion result - {:?}", insertion);

  Ok(count)
}
