use async_std::io::WriteExt;
use async_std::stream::StreamExt;
use clap::Parser;
use serde::Deserialize;
use std::io::{Error, ErrorKind, Result};

#[derive(Deserialize, Debug, Clone)]
struct UpdaterConfigArtifactNaming {
  starts_with: String,
  ends_with: String,
}

#[derive(Deserialize, Debug, Clone)]
struct UpdaterConfigExtractionRule {
  destination: String,
}

#[derive(Deserialize, Debug, Clone)]
struct UpdaterConfigSystemdRule {
  service: Option<String>,
  services: Option<Vec<String>>,
}

#[derive(Deserialize, Debug, Clone)]
struct GithubUpdaterConfig {
  name: String,
  repo: String,
  semver_storage: String,
  artifact_naming: UpdaterConfigArtifactNaming,
  extraction: UpdaterConfigExtractionRule,
  systemd: Option<UpdaterConfigSystemdRule>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(tag = "kind")]
enum UpdaterUnitConfig {
  #[serde(rename = "github-release-tarball")]
  GithubRelease(GithubUpdaterConfig),
}

impl UpdaterUnitConfig {
  fn name(&self) -> String {
    match self {
      UpdaterUnitConfig::GithubRelease(inner) => inner.name.clone(),
    }
  }
}

#[derive(Deserialize, Debug, Clone)]
struct UpdaterServerConfig {
  addr: String,
}

#[derive(Deserialize, Debug, Clone)]
struct UpdaterPollerConfig {
  delay_seconds: u64,
}

#[derive(Deserialize, Debug, Clone)]
struct UpdaterConfig {
  units: Option<Vec<UpdaterUnitConfig>>,
  poller: UpdaterPollerConfig,
  server: Option<UpdaterServerConfig>,
}

#[derive(Parser, Deserialize)]
#[clap(author, version, about, long_about = None)]
struct UpdaterCommandLineOptions {
  #[clap(short, long, value_parser)]
  config: String,

  #[clap(short, long, value_parser)]
  run_immediately: bool,
}

#[derive(Deserialize, Debug)]
struct GithubReleaseLatestResponseAsset {
  id: u32,
  name: String,
  url: String,
}

#[derive(Deserialize, Debug)]
struct GithubReleaseLatestResponse {
  id: u32,
  name: String,
  assets: Vec<GithubReleaseLatestResponseAsset>,
}

#[derive(Debug, Default)]
enum InitialRunFlags {
  Update,

  ToVersion(String),

  #[default]
  Nothing,
}

#[derive(Debug, Default)]
enum ManualRunRequest {
  #[default]
  All,

  Specific(String, Option<String>),
}

#[derive(Deserialize, Debug)]
struct ManualRunRequestPayload {
  name: Option<String>,
  version: Option<String>,
}

#[derive(Debug)]
enum UpdaterUnitResult {
  Updated,
  Nothing,
}

async fn github_release(config: &GithubUpdaterConfig, flags: &InitialRunFlags) -> Result<UpdaterUnitResult> {
  let run_id = uuid::Uuid::new_v4();
  log::debug!("unit '{}' running @ {:?}", config.name, run_id);

  let auth_token = std::env::var("GITHUB_RELEASE_AUTH_TOKEN")
    .map_err(|_| Error::new(ErrorKind::Other, format!("missing 'GITHUB_RELEASE_AUTH_TOKEN'")))?;

  if let InitialRunFlags::Update = flags {
    log::warn!("{run_id} force initial run");
  }

  // Attempt to parse the contents of the `semver_storage` configuration option as a valid sematic
  // version string. We will be using this to compare with the cloud value, and updating it to any
  // installed version on successful download.
  let version_data = async_std::fs::read(&config.semver_storage)
    .await
    .map_err(|error| log::warn!("{error}"))
    .ok();

  let current_version = version_data
    .and_then(|data| String::from_utf8(data).ok())
    .and_then(|bytes| semver::Version::parse(bytes.trim_start_matches("v")).ok());

  log::debug!("current version - {current_version:?}");

  let normalized = config.repo.trim_start_matches("https://github.com/");
  let url = format!("https://api.github.com/repos/{normalized}");

  log::debug!("running github release update check (@ {url})");

  // Depending on whether or not we're updating to a specific version, we will either use a
  // tag-specific api endpoint, or the `/latest`, which will return information about the latest
  // release.
  let release_url = match flags {
    InitialRunFlags::ToVersion(version) => format!("{}/releases/tags/{}", url, version),
    _ => format!("{}/releases/latest", url),
  };

  // Use the `/releases/latest` api route to fetch whatever release was latest.
  let mut response = surf::get(&release_url)
    .header("Authorization", format!("token {auth_token}"))
    .await
    .map_err(|error| Error::new(ErrorKind::Other, format!("failed to fetch latest - {error}")))
    .and_then(|res| {
      if res.status().is_success() {
        Ok(res)
      } else {
        let reason = res.status().canonical_reason();
        log::warn!("unable to find '{}' -> {reason}", release_url);
        Err(Error::new(ErrorKind::Other, format!("{reason}")))
      }
    })?;

  log::debug!("response loaded ({})", response.status().canonical_reason());

  let latest = response
    .body_json::<GithubReleaseLatestResponse>()
    .await
    .map_err(|error| Error::new(ErrorKind::Other, format!("unable to parse github response - {error}")))?;

  log::debug!(
    "found latest relase {} '{}' ({} assets)",
    latest.id,
    latest.name,
    latest.assets.len()
  );

  // Attempt to parse the github response payload's `name` field as a semantic version. This
  // ultimately means that we require all release names to be a semantic verison.
  let should_update = match (
    flags,
    current_version,
    semver::Version::parse(latest.name.trim_start_matches("v")).ok(),
  ) {
    (InitialRunFlags::Update, _, _) | (InitialRunFlags::ToVersion(_), _, _) => true,

    (_, None, Some(next)) => {
      log::warn!("no current version found, assuming valid update to {next}");
      true
    }

    (_, Some(current), Some(next)) if next > current => {
      log::info!("new version found ({current} -> {next})");
      true
    }

    (_, Some(current), Some(next)) if next == current => {
      log::debug!("currently up to date");
      false
    }

    _ => {
      log::warn!("unable to determine semantic versioning comparison, doing nothing");
      false
    }
  };

  if should_update == false {
    return Ok(UpdaterUnitResult::Nothing);
  }

  log::info!("proceeding with update, checking destination");

  let asset = match latest.assets.into_iter().find(|item| {
    let match_prefix = item.name.starts_with(&config.artifact_naming.starts_with);
    let match_suffix = item.name.ends_with(&config.artifact_naming.ends_with);
    match_prefix && match_suffix
  }) {
    Some(inner) => inner,
    None => {
      log::warn!(
        "no assets maching '{}' '{}' config, skipping",
        config.artifact_naming.starts_with,
        config.artifact_naming.ends_with
      );
      return Ok(UpdaterUnitResult::Nothing);
    }
  };

  log::info!("found artifact match {} ('{}' @ '{}')", asset.id, asset.name, asset.url);

  // We're assuming an invariant here that the github api for assets will always attempt to return
  // a redirect when requesting the data for a specific asset.
  let locate_response = surf::get(&asset.url)
    .header("Accept", "application/octet-stream")
    .header("Authorization", format!("token {}", auth_token))
    .await
    .map_err(|error| Error::new(ErrorKind::Other, format!("failed to fetch latest - {error}")))
    .and_then(|res| {
      if res.status() == surf::StatusCode::Found {
        Ok(res)
      } else {
        let reason = res.status().canonical_reason();
        Err(Error::new(ErrorKind::Other, format!("{reason}")))
      }
    })?;

  let real_location = locate_response
    .header("Location")
    .and_then(|value| value.get(0))
    .map(|value| value.as_str().to_string())
    .ok_or_else(|| Error::new(ErrorKind::Other, "bad response from github"))?;

  log::info!("found real location ({}), downloading.", real_location);

  let mut download_response = surf::get(&real_location)
    .await
    .map_err(|error| Error::new(ErrorKind::Other, format!("failed to fetch latest - {error}")))
    .and_then(|res| {
      if res.status().is_success() {
        Ok(res)
      } else {
        let reason = res.status().canonical_reason();
        Err(Error::new(ErrorKind::Other, format!("{reason}")))
      }
    })?;

  log::info!("download headers received, receiving bytes");

  // Create two tasks/futures that will resolve when the entire request body has been loaded into
  // memory. One actually polls the reading and one writes to our logs.
  let (sender, receiver) = async_std::channel::bounded(1);

  let download_future = async {
    let result = download_response.body_bytes().await;
    log::info!("finished download");
    let _ = sender.send(1).await;
    drop(sender);
    result
  };

  let writer_future = async {
    let mut now = std::time::Instant::now();

    loop {
      async_std::task::sleep(std::time::Duration::from_millis(50)).await;

      if now.elapsed().as_secs() >= 2 {
        log::info!("still downloading...");
        now = std::time::Instant::now();
      }

      if let Err(async_std::channel::TryRecvError::Closed) = receiver.try_recv() {
        break;
      }
    }

    true
  };

  let (bytes, _) = futures_lite::future::zip(download_future, writer_future).await;

  // The future awaited up to this point only implies that the header section of our http request
  // has been received. To actually load the contents of the asset file, we need to read the rest
  // of the request into memory.
  let bytes = bytes.map_err(|error| {
    log::warn!("{error}");
    Error::new(ErrorKind::Other, "failed-download")
  })?;

  // Prepare a temporary directory that our tarball will be unpacked into.
  let temp_dir = std::env::temp_dir().join("beetle-updater").join(&run_id.to_string());
  log::info!("download complete, creating temp dir {:?}", temp_dir);
  async_std::fs::create_dir_all(&temp_dir).await?;

  let decompressor = flate2::read::GzDecoder::new(bytes.as_slice());
  let mut archive = tar::Archive::new(decompressor);

  let entries = archive
    .entries()
    .map_err(|error| Error::new(ErrorKind::Other, format!("invalid-format: {error}")))?;

  for entry in entries {
    let mut entry = entry?;
    let path = entry.path()?;
    let entry_destination = temp_dir.join(&path);
    log::info!("found entry {:?} -> {:?}", &path, entry_destination);
    entry.unpack(&entry_destination)?;
  }

  // Add the version immedaitely after the configured destination to get the final output directory
  // that we will move our artifact into from the `tmp` directory.
  let full_buff = std::path::PathBuf::from(&config.extraction.destination);
  let full_destination_path = full_buff.join(&latest.name);
  let latest_destination_path = full_buff.join("latest");

  // We're going to be using symbolic links to manager having multiple versions living
  // simultaneously on the same machine.
  match async_std::fs::metadata(&latest_destination_path).await {
    Ok(meta) if meta.is_file() || meta.is_symlink() => {
      log::warn!("cleaning up FILE existing content at '{:?}'", latest_destination_path);
      async_std::fs::remove_file(&latest_destination_path).await?
    }
    Ok(meta) if meta.is_dir() => {
      log::warn!("cleaning up DIR existing content at '{:?}'", latest_destination_path);
      async_std::fs::remove_dir_all(&latest_destination_path).await?
    }
    Ok(meta) => return Err(Error::new(ErrorKind::Other, format!("unknown file stat - {meta:?}"))),

    Err(error) if error.kind() == ErrorKind::NotFound => log::info!("existing symlink not found, moving on"),
    Err(error) => return Err(error),
  }

  // Check the location that we will be moving to. If it exists, it is likely that we are force
  // updating the same version on our machine.
  match async_std::fs::metadata(&full_destination_path).await {
    Ok(_) => {
      let mut backup_path = full_destination_path.clone();
      backup_path.pop();
      backup_path = backup_path.join(format!("{}-backup-{run_id}", latest.name));

      log::warn!("{:?} exists, backup to {:?}", full_destination_path, backup_path);

      async_std::fs::rename(&full_destination_path, &backup_path)
        .await
        .map_err(|error| {
          log::warn!("unable to cleanup matching old contents - {error}");
          error
        })?;
    }
    Err(error) if error.kind() == ErrorKind::NotFound => {
      log::debug!("nothing at {:?}, moving on", full_destination_path)
    }
    Err(error) => return Err(error),
  }

  log::info!(
    "renaming download to '{:?}' (symlink to {:?})",
    full_destination_path,
    latest_destination_path
  );

  // TODO: maybe consider making this safer; the goal is to make sure all parent directories have
  // been created before we attempt to put our download from `tmp` to the full path.
  async_std::fs::create_dir_all(&full_destination_path)
    .await
    .map_err(|error| {
      log::warn!("uanble to create '{:?}' - {error}", full_destination_path);
      error
    })?;

  async_std::fs::rename(&temp_dir, &full_destination_path)
    .await
    .map_err(|error| {
      log::warn!(
        "uanble to rename '{:?}' -> '{:?}' - {error}",
        temp_dir,
        full_destination_path
      );
      error
    })?;

  async_std::os::unix::fs::symlink(&full_destination_path, &latest_destination_path)
    .await
    .map_err(|error| {
      log::warn!(
        "unable to create symlink '{:?}' -> '{:?}' - {error}",
        full_destination_path,
        latest_destination_path
      );
      error
    })?;

  // Update the version storage file with our new release name.
  log::info!("success, writing new version to storage");
  let mut file = async_std::fs::File::create(&config.semver_storage).await?;
  async_std::write!(&mut file, "{}", latest.name).await?;

  let services = config
    .systemd
    .as_ref()
    .and_then(|systemd_config| {
      systemd_config
        .service
        .as_ref()
        .map(|service| vec![service.clone()])
        .or(systemd_config.services.clone())
    })
    .unwrap_or_default();

  for service in services {
    log::debug!("attempting to restart service '{service}'");

    let output = async_std::process::Command::new("systemctl")
      .arg("restart")
      .arg(&service)
      .output()
      .await?;

    if output.status.success() {
      log::info!("successfully restarted '{service}'");
      return Ok(UpdaterUnitResult::Updated);
    }

    log::warn!("unable to restart service - {:?}", String::from_utf8(output.stderr));
  }

  Ok(UpdaterUnitResult::Updated)
}

async fn run(mut config: UpdaterConfig, receiver: async_std::channel::Receiver<ManualRunRequest>) -> Result<()> {
  let mut interval = async_std::stream::interval(std::time::Duration::from_secs(config.poller.delay_seconds));

  log::info!("entering working loop for config: {config:#?}");

  loop {
    interval.next().await;

    // Attempt to pull a message from our receiver in a non-blocking way.
    let message = receiver
      .try_recv()
      .map(Some)
      .or_else(|error| if error.is_closed() { Err(error) } else { Ok(None) })
      .map_err(|error| Error::new(ErrorKind::Other, format!("web listener closed - {error}")))?;

    let units = config
      .units
      .unwrap_or_default()
      .drain(0..)
      .collect::<Vec<UpdaterUnitConfig>>();

    let mut next = Vec::with_capacity(units.len());

    for unit in units {
      // Check to see if we have a message that is requesting this specific unit to run.
      let flags = match &message {
        Some(ManualRunRequest::Specific(name, ref version_kind)) if name == &unit.name() => version_kind
          .as_ref()
          .map(|version| InitialRunFlags::ToVersion(version.clone()))
          .unwrap_or(InitialRunFlags::Update),

        Some(ManualRunRequest::All) => InitialRunFlags::Update,

        _ => InitialRunFlags::Nothing,
      };

      // Match on the unit kind and run it!
      let result = match unit {
        UpdaterUnitConfig::GithubRelease(ref config) => github_release(&config, &flags).await,
      };

      log::info!("unit '{}' check -> {result:?}", unit.name());

      if let Err(error) = result {
        log::warn!("updater failed on unit - '{}' - {error}", unit.name());
        continue;
      }

      next.push(unit);
    }

    if next.len() == 0 {
      return Err(Error::new(ErrorKind::Other, "no units left"));
    }

    config.units = Some(next);
  }
}

async fn attempt_run(mut request: tide::Request<async_std::channel::Sender<ManualRunRequest>>) -> tide::Result {
  log::info!("handling run api request");

  let body = request.body_json::<ManualRunRequestPayload>().await?;

  let message = match (body.name, body.version) {
    (None, None) => ManualRunRequest::All,
    (Some(name), None) => ManualRunRequest::Specific(name, None),
    (Some(name), Some(version)) => ManualRunRequest::Specific(name, Some(version)),
    (None, Some(_)) => {
      log::warn!("received version withhout a name, skipping");
      return Err(tide::Error::from_str(422, "missing 'name' for specific version"));
    }
  };

  log::info!("parsed request body - {message:?}");

  if let Err(error) = request.state().send(message).await {
    log::warn!("unable to send from api - {error}");
  }

  Ok(tide::Response::new(200))
}

async fn listen(config: UpdaterServerConfig, sink: async_std::channel::Sender<ManualRunRequest>) -> Result<()> {
  let mut app = tide::with_state(sink.clone());

  app.at("/run").post(attempt_run);

  let addr = &config.addr;
  log::info!("http server listening on '{}'", addr);

  // Start our async web listener while _also_ spawning a future that will resolve if our sink
  // channel is ever closed.
  futures_lite::future::race(app.listen(addr), async {
    let mut interval = async_std::stream::interval(std::time::Duration::from_millis(10));

    loop {
      interval.next().await;

      if sink.is_closed() {
        log::warn!("manual run sink appears closed, terminating api");
        return Ok(());
      }
    }
  })
  .await
}

fn main() -> Result<()> {
  let _ = dotenv::dotenv();
  env_logger::init();
  log::info!("env loaded");

  let options = UpdaterCommandLineOptions::parse();
  let ex = async_executor::LocalExecutor::new();
  let config_content = std::fs::read(&options.config)?;
  let config = toml::from_slice::<UpdaterConfig>(&config_content)?;

  let (run_sender, run_receiver) = async_std::channel::unbounded();

  let listener_future = async {
    if options.run_immediately {
      log::warn!("running immediately, sending message");

      run_sender.send(ManualRunRequest::All).await.map_err(|error| {
        log::warn!("unable to sent initial run - {error}");
        Error::new(ErrorKind::Other, format!("bad send - {error}"))
      })?;
    }

    match config.clone().server.take() {
      Some(config) => listen(config, run_sender).await,
      None => Ok(()),
    }
  };

  let runner_future = run(config.clone(), run_receiver);
  let zipped_futures = futures_lite::future::zip(runner_future, listener_future);
  let (runner_result, listener_result) = futures_lite::future::block_on(ex.run(zipped_futures));
  runner_result.and(listener_result)
}
