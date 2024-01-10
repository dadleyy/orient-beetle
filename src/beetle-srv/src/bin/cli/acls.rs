use std::io;

use clap::Parser;
use serde::Deserialize;

/// When provisioning the redis acl entries, this command holds the username and password that will
/// be used by devices to request their unique id.
#[derive(Parser, Deserialize, PartialEq)]
pub struct ProvisionCommand {
  /// The user portion of our id-requesting `AUTH` creds.
  user: Option<String>,
  /// The password portion of our id-requesting `AUTH` creds.
  password: Option<String>,
}

/// A almost useless helper function that parses an acl entry response from redis and extracts the
/// device id from it.
fn id_from_acl_entry(entry: &str) -> Option<&str> {
  entry.split(' ').nth(1)
}

/// Creates the initial set of acl entries that will be used at burn-in by devices.
pub async fn provision(config: &super::CommandLineConfig, command: ProvisionCommand) -> io::Result<()> {
  let ProvisionCommand { user, password } = command;
  let mut stream = beetle::redis::connect(&config.redis).await?;

  let password = password.or(config.registrar.id_consumer_password.clone());
  let user = user.or(config.registrar.id_consumer_username.clone());
  println!("provisioning redis environment with burn-in auth information (p: {password:?}, u: {user:?})");

  match (user, password) {
    (Some(ref user), Some(ref pass)) if !user.is_empty() && !pass.is_empty() => {
      let command = kramer::Command::Acl::<&str, &str>(kramer::acl::AclCommand::SetUser(kramer::acl::SetUser {
        name: user,
        password: Some(pass),
        keys: Some(beetle::constants::REGISTRAR_AVAILABLE),
        commands: Some(vec!["lpop", "blpop"]),
      }));

      log::debug!("sending {command:?}");
      let result = kramer::execute(&mut stream, &command).await;
      log::info!("acl provisioning result - {result:?}");
      println!("ok");
    }
    _ => {
      return Err(io::Error::new(
        io::ErrorKind::Other,
        "username or pasword missing for provisioning",
      ));
    }
  }

  Ok(())
}

/// Finds all acl entries on our redis instance and prints them.
pub async fn print_acls(config: &super::CommandLineConfig) -> io::Result<()> {
  let mut stream = beetle::redis::connect(&config.redis).await?;
  let allowed: std::collections::hash_set::HashSet<String> = match &config.registrar.acl_user_allowlist {
    Some(ref list) => std::collections::hash_set::HashSet::from_iter(list.iter().cloned()),
    None => {
      log::warn!("no acl allowlist configured");
      std::collections::hash_set::HashSet::new()
    }
  };
  log::debug!("looking for acl entries to destroy, skipping {allowed:?}");
  let list = kramer::execute(&mut stream, kramer::Command::Acl::<u8, u8>(kramer::AclCommand::List)).await;

  let values = match list {
    Ok(kramer::Response::Array(inner)) => inner,
    _ => return Err(io::Error::new(io::ErrorKind::Other, "")),
  };

  let names = values
    .into_iter()
    .filter_map(|entry| match entry {
      kramer::ResponseValue::String(v) => {
        let id = id_from_acl_entry(&v)?;
        log::trace!("found id {id}");

        if allowed.contains(id) {
          None
        } else {
          Some(id.to_string())
        }
      }
      _ => None,
    })
    .collect::<Vec<String>>();

  if names.is_empty() {
    println!("no matching acl entries to delete");
    return Ok(());
  }

  for name in names {
    println!("{name}");
  }

  Ok(())
}

/// Finds all acl entries on our redis instance and deletes them.
pub async fn invalidate_acls(config: &super::CommandLineConfig) -> io::Result<()> {
  let mut stream = beetle::redis::connect(&config.redis).await?;
  let allowed: std::collections::hash_set::HashSet<String> = match &config.registrar.acl_user_allowlist {
    Some(ref list) => std::collections::hash_set::HashSet::from_iter(list.iter().cloned()),
    None => std::collections::hash_set::HashSet::new(),
  };
  log::debug!("looking for acl entries to destroy, skipping {allowed:?}");
  let list = kramer::execute(&mut stream, kramer::Command::Acl::<u8, u8>(kramer::AclCommand::List)).await;

  let values = match list {
    Ok(kramer::Response::Array(inner)) => inner,
    _ => return Err(io::Error::new(io::ErrorKind::Other, "")),
  };

  let names = values
    .into_iter()
    .filter_map(|entry| match entry {
      kramer::ResponseValue::String(v) => {
        let id = id_from_acl_entry(&v)?;
        log::trace!("found id {id}");

        if allowed.contains(id) {
          None
        } else {
          Some(id.to_string())
        }
      }
      _ => None,
    })
    .collect::<Vec<String>>();

  if names.is_empty() {
    println!("no matching acl entries to delete");
    return Ok(());
  }

  println!("the following acl entries will be deleted. enter 'y' to continue: {names:?}");
  let mut buffer = String::new();
  io::stdin().read_line(&mut buffer)?;

  if buffer.as_str().trim_end() != "y" {
    println!("aborting.");
    return Ok(());
  }

  // Delete the ACL entries _before_ the queue. This is important so the registrar worker is
  // does not refill acl entries that would be immediate destroyed.
  log::info!("continuing with deletion");
  let command = kramer::Command::<String, &str>::Acl(kramer::AclCommand::DelUser(kramer::Arity::Many(names)));
  kramer::execute(&mut stream, &command).await?;

  log::info!("now clearing off our registration queue");
  let command = kramer::Command::<&str, &str>::Del(kramer::Arity::One(beetle::constants::REGISTRAR_AVAILABLE));
  kramer::execute(&mut stream, &command).await?;
  println!("done.");
  Ok(())
}
