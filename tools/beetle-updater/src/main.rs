#![warn(clippy::missing_docs_in_private_items)]

//! This project is meant to run as a daemon that is able to "update"
//! other background applications running on linux machines. Currently,
//! the only supported method involves downloading a github release
//! artifact, unpacking its contents and restarting a systemd unit.

use async_std::io::WriteExt;
use async_std::stream::StreamExt;
use clap::Parser;
use serde::Deserialize;
use std::io::{Error, ErrorKind, Result};

/// When using the github updater configuration, this struct defines a
/// "filter" that will be used to find a specific artifact in the set
/// of artifacts associated with a release; effectively acting as a
/// dumb version of a regex.
#[derive(Deserialize, Debug, Clone)]
struct UpdaterConfigArtifactNaming {
  /// The hope here is that the artifacts uploaded to github share a
  /// common prefix across all release versions.
  starts_with: String,

  /// The hope here is that the artifacts uploaded to github share a
  /// common suffix across all release versions.
  ends_with: String,
}

/// When using the github updater configuration, we are expecting that
/// the release artifact is a compressed (tar.gz) file that needs to be
/// extracted somewhere.
#[derive(Deserialize, Debug, Clone)]
struct UpdaterConfigExtractionRule {
  /// The location on the machine running our daemon that we will extract
  /// the github release artifact to.
  destination: String,
}

/// This daemon is currently built around the idea of replacing the single
/// executable underneath a running systemd unit and restarting it once
/// complete.
#[derive(Deserialize, Debug, Clone)]
struct UpdaterConfigSystemdRule {
  /// The name of the service (single) to restart.
  service: Option<String>,

  /// The name of the services (multiple) to restart.
  services: Option<Vec<String>>,
}

/// The primary method of configuring our updater, the github updater config
/// expects that we will be downloading an artifact attatched to a release that
/// has been named using semantic versioning, extracting its contents and
/// restarting a systemd unit.
#[derive(Deserialize, Debug, Clone)]
struct GithubUpdaterConfig {
  /// Our configurations will be stored in a `Hash<name, ...>`; it should be unique.
  name: String,

  /// The name of the repo; this _will_ be used for url generation.
  repo: String,

  /// The location on disk where we can write the last updated version to.
  semver_storage: String,

  /// Filtering conditions to use when deciding what artifact to download.
  artifact_naming: UpdaterConfigArtifactNaming,

  /// What to do with the artifact (how to extract, where to put)
  extraction: UpdaterConfigExtractionRule,

  /// What services to restart.
  systemd: Option<UpdaterConfigSystemdRule>,
}

/// Over time, this is built to support multiple kinds of updates.
#[derive(Deserialize, Debug, Clone)]
#[serde(tag = "kind")]
enum UpdaterUnitConfig {
  /// The primary updater method; github.
  #[serde(rename = "github-release-tarball")]
  GithubRelease(GithubUpdaterConfig),
}

impl UpdaterUnitConfig {
  /// Every configuration variant should be able to be uniquely identified amongst a
  /// set of multiple configurations.
  fn name(&self) -> String {
    match self {
      UpdaterUnitConfig::GithubRelease(inner) => inner.name.clone(),
    }
  }
}

/// By default, the daemon will only "queue" an update if a discrepancy is found. The
/// server, listening on this address provided by configuration, is what a client will
/// interact with to actually kick off an update.
#[derive(Deserialize, Debug, Clone)]
struct UpdaterServerConfig {
  /// The address our tcp listener will bind to.
  addr: String,
}

/// Poller configuration.
#[derive(Deserialize, Debug, Clone)]
struct UpdaterPollerConfig {
  /// How long to wait between attempts to check version discrepancies.
  delay_seconds: u64,
}

/// This struct represents the "complete" configuration schema we are expecting from the
/// toml file.
#[derive(Deserialize, Debug, Clone)]
struct UpdaterConfig {
  /// Each unit represents a "strategy" for updating. This could include multiple
  /// applications.
  units: Option<Vec<UpdaterUnitConfig>>,

  /// Poller configuration.
  poller: UpdaterPollerConfig,

  /// Server configuration.
  server: Option<UpdaterServerConfig>,
}

/// Command line options struct, provided by clap.
#[derive(Parser, Deserialize)]
#[clap(author, version = option_env!("BEETLE_UPDATER_VERSION").unwrap_or("dev"), about, long_about = None)]
struct UpdaterCommandLineOptions {
  /// The location we will use to open a and deserialize our configuration.
  #[clap(short, long, value_parser)]
  config: String,

  /// If true, will attempt a "run" (checking version discrepency) immediately.
  #[clap(short, long, value_parser)]
  run_immediately: bool,
}

/// The schema of a github api response for fetching a single asset.
#[derive(Deserialize, Debug)]
struct GithubReleaseLatestResponseAsset {
  /// A github id
  id: u32,

  /// A string
  name: String,

  /// A url
  url: String,
}

/// The schema of a github api response for fetching the latest release.
#[derive(Deserialize, Debug)]
struct GithubReleaseLatestResponse {
  /// Provided by github.
  id: u32,

  /// The name of the release, hopefully something semver>
  name: String,

  /// The list of assets attached to the release.
  assets: Vec<GithubReleaseLatestResponseAsset>,
}

/// This enum represents the various "things" our updater should do during a single
/// run through of an updater strategy.
#[derive(Debug, Default)]
enum SingleRunExecutionStrategy {
  /// When run with this "strategy", any version discrepency will be subject to an
  /// update immediately.
  Update,

  /// When run with this "strategy", the updater will attempt to explicitly move to
  /// the version provided, ignoring version relationships entirely.
  ToVersion(String),

  /// This strategy indicates to the updater that it should only check for version
  /// discrepencies and _notify_ the daemon.
  #[default]
  Nothing,
}

/// When receiving requests from a user through our server, this enum represents the
/// various things we're allowing them to do.
#[derive(Debug, Default)]
enum ManualRunRequest {
  /// When provided, the daemon will run all of it's known updater units.
  #[default]
  All,

  /// When provided, the daemon will run only the specific unit matching the first
  /// member of our tuple, to the _optionally_ provided version in the second
  /// positiong.
  Specific(String, Option<String>),
}

/// The JSON schema of our api request.
#[derive(Deserialize, Debug)]
struct ManualRunRequestPayload {
  /// The name of our unit; if omitted, expect all units to run.
  name: Option<String>,

  /// The version to update to.
  version: Option<String>,
}

/// A given run attempt will end with various results.
#[derive(Debug)]
enum UpdaterUnitResult {
  /// The updated result here means that a new version has been applied to all
  /// applications included in our unit.
  Updated,

  /// When run without a specific version to apply, the daemon will return this from
  /// a run attempt if there was a version discrepancy.
  UpdateAvailable(String),

  /// If nothing, no new version is available, and nothing was applied.
  Nothing,
}

/// The main function for our github release units.
async fn github_release(config: &GithubUpdaterConfig, flags: &SingleRunExecutionStrategy) -> Result<UpdaterUnitResult> {
  let run_id = uuid::Uuid::new_v4();
  log::debug!("unit '{}' running @ {:?}", config.name, run_id);

  let auth_token = std::env::var("GITHUB_RELEASE_AUTH_TOKEN")
    .map_err(|_| Error::new(ErrorKind::Other, "missing 'GITHUB_RELEASE_AUTH_TOKEN'".to_string()))?;

  if let SingleRunExecutionStrategy::Update = flags {
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
    .and_then(|data| {
      String::from_utf8(data)
        .map_err(|error| {
          log::warn!("unable to parse version storage - {error}");
        })
        .ok()
    })
    .and_then(|bytes| {
      semver::Version::parse(bytes.trim_start_matches('v').trim_end())
        .map_err(|error| {
          log::warn!("unable to parse semver - {error}");
        })
        .ok()
    });

  log::debug!("current version - {current_version:?}");

  let normalized = config.repo.trim_start_matches("https://github.com/");
  let url = format!("https://api.github.com/repos/{normalized}");

  log::debug!("running github release update check (@ {url})");

  // Depending on whether or not we're updating to a specific version, we will either use a
  // tag-specific api endpoint, or the `/latest`, which will return information about the latest
  // release.
  let release_url = match flags {
    SingleRunExecutionStrategy::ToVersion(version) => format!("{}/releases/tags/{}", url, version),
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
        Err(Error::new(ErrorKind::Other, reason.to_string()))
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
    semver::Version::parse(latest.name.trim_start_matches('v')).ok(),
  ) {
    (SingleRunExecutionStrategy::Update, _, _) | (SingleRunExecutionStrategy::ToVersion(_), _, _) => true,

    (_, None, Some(next)) => {
      log::warn!("no current version found, assuming valid update to {next}");
      return Ok(UpdaterUnitResult::UpdateAvailable(latest.name.clone()));
    }

    // If we had a valid update via comparing versions (and this wasn't an explicit update), return
    // it back to the polling loop to queue for later.
    (_, Some(current), Some(next)) if next > current => {
      log::info!("new version found ({current} -> {next})");
      return Ok(UpdaterUnitResult::UpdateAvailable(latest.name.clone()));
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

  if !should_update {
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
        Err(Error::new(ErrorKind::Other, reason.to_string()))
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
        Err(Error::new(ErrorKind::Other, reason.to_string()))
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

  let services = config
    .systemd
    .as_ref()
    .and_then(|systemd_config| {
      systemd_config
        .service
        .as_ref()
        .map(|service| vec![service.clone()])
        .or_else(|| systemd_config.services.clone())
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
      continue;
    }

    log::warn!("unable to restart service - {:?}", String::from_utf8(output.stderr));
  }

  // Update the version storage file with our new release name.
  log::info!("success, writing new version to storage");
  let mut file = async_std::fs::File::create(&config.semver_storage).await?;
  async_std::write!(&mut file, "{}", latest.name).await?;

  Ok(UpdaterUnitResult::Updated)
}

/// The async runtime here is responsible for executing units and pulling messages off
/// the channel being written to from the api.
async fn run(
  mut config: UpdaterConfig,
  receiver: async_std::channel::Receiver<ManualRunRequest>,
  updates: async_std::channel::Sender<VersionMessage>,
) -> Result<()> {
  let mut interval = async_std::stream::interval(std::time::Duration::from_millis(500));
  let mut now = std::time::Instant::now();

  log::info!("entering working loop for config: {config:#?}");

  loop {
    interval.next().await;

    // Attempt to pull a message from our receiver in a non-blocking way.
    let message = receiver
      .try_recv()
      .map(Some)
      .or_else(|error| if error.is_closed() { Err(error) } else { Ok(None) })
      .map_err(|error| Error::new(ErrorKind::Other, format!("web listener closed - {error}")))?;

    // Note: we don't want to actually sleep for the time specified by the user in their
    // configuration; if we did that we may take longer to receive messages from our http server
    // than we want to.
    if now.elapsed().as_secs() < config.poller.delay_seconds && message.is_none() {
      continue;
    }

    now = std::time::Instant::now();

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
          .map(|version| SingleRunExecutionStrategy::ToVersion(version.clone()))
          .unwrap_or(SingleRunExecutionStrategy::Update),

        Some(ManualRunRequest::All) => SingleRunExecutionStrategy::Update,

        _ => SingleRunExecutionStrategy::Nothing,
      };

      // Match on the unit kind and run it!
      let result = match unit {
        UpdaterUnitConfig::GithubRelease(ref config) => github_release(config, &flags).await,
      };

      log::info!("unit '{}' check -> {result:?}", unit.name());

      match result {
        Err(error) => {
          log::warn!("updater failed on unit - '{}' - {error}", unit.name());
          continue;
        }
        Ok(UpdaterUnitResult::UpdateAvailable(version)) => {
          log::info!("an update '{version}' is available");
          updates
            .send(VersionMessage::HasVersion(unit.name(), version))
            .await
            .map_err(|error| {
              log::warn!("unable to send update to api from worker - {error}");
              Error::new(ErrorKind::Other, format!("{error}"))
            })?;
        }
        Ok(other) => {
          if let Err(error) = updates.send(VersionMessage::NoVersion).await {
            log::warn!("unable to send version message to web - {error}");
            continue;
          }
          log::info!("{} completed with {other:?}", unit.name())
        }
      }

      next.push(unit);
    }

    if next.is_empty() {
      return Err(Error::new(ErrorKind::Other, "no units left"));
    }

    config.units = Some(next);
  }
}

/// The channel used to communicate new versions between our runner and the web thread.
#[derive(Debug, Clone)]
enum VersionMessage {
  /// The runner has found a new version for a specific unit/service.
  HasVersion(String, String),

  /// The runner has not found a new version for a specific unit/service.
  NoVersion,
}

/// The webcontext here is a thread-safe structure that can be passed between tcp connections
/// established by `tide` (our web thread).
#[derive(Clone)]
struct WebContext(
  async_std::channel::Sender<ManualRunRequest>,
  async_std::sync::Arc<async_std::sync::Mutex<Option<(String, String)>>>,
);

/// Route: attempts to send/queue the version that is currently stored in our web context mutex
/// back into the run request sink.
async fn post_commit_update(request: tide::Request<WebContext>) -> tide::Result {
  log::info!("getting available version");
  let WebContext(sender, version_mutex) = request.state();

  let mut version = version_mutex.lock().await;

  match version.take() {
    Some((service, version)) => {
      if let Err(error) = sender
        .send(ManualRunRequest::Specific(service.clone(), Some(version.clone())))
        .await
      {
        log::warn!("unable to send queued update - {error}");
      }

      Ok(format!("{version:?}").into())
    }
    None => Ok(tide::Response::new(404)),
  }
}

/// Route: returns any available update stored in our web context mutex.
async fn get_available_update(request: tide::Request<WebContext>) -> tide::Result {
  log::info!("getting available version");
  let WebContext(_, version_mutex) = request.state();

  let version = version_mutex.lock().await;

  Ok(format!("{version:?}").into())
}

/// Route: accepts user input, will attempt to forceably run an update.
async fn post_attempt_run(mut request: tide::Request<WebContext>) -> tide::Result {
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

  let WebContext(sender, _) = request.state();

  if let Err(error) = sender.send(message).await {
    log::warn!("unable to send from api - {error}");
  }

  Ok(tide::Response::new(200))
}

/// The main async entrypoint for our web thread.
async fn listen(
  config: UpdaterServerConfig,
  sink: async_std::channel::Sender<ManualRunRequest>,
  updates: async_std::channel::Receiver<VersionMessage>,
) -> Result<()> {
  let version_mutex = async_std::sync::Arc::new(async_std::sync::Mutex::new(None));
  let mut app = tide::with_state(WebContext(sink.clone(), version_mutex.clone()));

  app.at("/run").post(post_attempt_run);
  app.at("/updates").get(get_available_update);
  app.at("/commit-update").post(post_commit_update);

  let addr = &config.addr;
  log::info!("http server listening on '{}'", addr);

  // Start our async web listener while _also_ spawning a future that will resolve if our sink
  // channel is ever closed.
  futures_lite::future::race(app.listen(addr), async {
    let mut interval = async_std::stream::interval(std::time::Duration::from_millis(10));

    loop {
      interval.next().await;

      match updates.try_recv() {
        Ok(VersionMessage::NoVersion) => {
          log::info!("web worker cleared update");
          let mut unlocked_version = version_mutex.lock().await;
          *unlocked_version = None;
        }

        Ok(VersionMessage::HasVersion(service, version)) => {
          log::info!("web worker received update - {version:?}");
          let mut unlocked_version = version_mutex.lock().await;
          *unlocked_version = Some((service, version));
        }

        Err(error) if error == async_std::channel::TryRecvError::Empty => {
          log::trace!("nothing to receive from update stream");
        }

        Err(error) => {
          log::warn!("unable to read from update channel - {error}");
          return Ok(());
        }
      }

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
  let (update_sender, update_receiver) = async_std::channel::unbounded();

  let listener_future = async {
    if options.run_immediately {
      log::warn!("running immediately, sending message");

      run_sender.send(ManualRunRequest::All).await.map_err(|error| {
        log::warn!("unable to sent initial run - {error}");
        Error::new(ErrorKind::Other, format!("bad send - {error}"))
      })?;
    }

    match config.clone().server.take() {
      Some(config) => listen(config, run_sender, update_receiver).await,
      None => Ok(()),
    }
  };

  let runner_future = run(config.clone(), run_receiver, update_sender);
  let zipped_futures = futures_lite::future::zip(runner_future, listener_future);
  let (runner_result, listener_result) = futures_lite::future::block_on(ex.run(zipped_futures));
  runner_result.and(listener_result)
}
