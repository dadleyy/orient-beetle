use async_std::io::WriteExt;
use async_std::stream::StreamExt;
use clap::Parser;
use serde::Deserialize;
use std::io::{Error, ErrorKind, Result};

#[derive(Deserialize, Debug)]
struct UpdaterConfigArtifactNaming {
  starts_with: String,
  ends_with: String,
}

#[derive(Deserialize, Debug)]
struct UpdaterConfigExtractionRule {
  destination: String,
}

#[derive(Deserialize, Debug)]
struct GithubUpdaterConfig {
  name: String,
  repo: String,
  semver_storage: String,
  artifact_naming: UpdaterConfigArtifactNaming,
  extraction: UpdaterConfigExtractionRule,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "kind")]
enum UpdaterUnitConfig {
  #[serde(rename = "github-release-tarball")]
  GithubRelease(GithubUpdaterConfig),
}

#[derive(Deserialize, Debug)]
struct UpdaterConfig {
  units: Option<Vec<UpdaterUnitConfig>>,
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
  #[default]
  Nothing,
}

async fn github_release(config: &GithubUpdaterConfig, flags: &InitialRunFlags) -> Result<()> {
  let run_id = uuid::Uuid::new_v4();
  log::debug!("unit '{}' running @ {:?}", config.name, run_id);

  let auth_token = std::env::var("GITHUB_RELEASE_AUTH_TOKEN")
    .map_err(|_| Error::new(ErrorKind::Other, format!("missing 'GITHUB_RELEASE_AUTH_TOKEN'")))?;

  if let InitialRunFlags::Update = flags {
    log::warn!("{run_id} force initial run");
  }

  let version_data = async_std::fs::read(&config.semver_storage)
    .await
    .map_err(|error| log::warn!("{error}"))
    .ok();

  let current_version = version_data
    .and_then(|data| String::from_utf8(data).ok())
    .and_then(|bytes| semver::Version::parse(bytes.trim_start_matches("v")).ok());

  log::debug!("current version - {current_version:?}");

  let normalized = config.repo.trim_end_matches("https://github.com/");
  let url = format!("https://api.github.com/repos/{normalized}");

  log::debug!("running github release update check for {config:?} (@ {url})");

  let mut response = surf::get(format!("{}/releases/latest", url))
    .header("Authorization", format!("token {auth_token}"))
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

  let should_update = match (
    flags,
    current_version,
    semver::Version::parse(latest.name.trim_start_matches("v")).ok(),
  ) {
    (InitialRunFlags::Update, _, _) => true,

    (_, Some(current), Some(next)) if next > current => {
      log::info!("new version found");
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
    return Ok(());
  }

  log::info!("proceeding with update, checking destination");

  match async_std::fs::metadata(&config.extraction.destination).await {
    Ok(meta) if meta.file_type().is_dir() => {
      let backup = format!("{}.{}.bak", &config.extraction.destination, run_id);
      log::warn!("{} exists, backing up to {}", config.extraction.destination, backup);
      async_std::fs::rename(&config.extraction.destination, &backup).await?;
    }
    Err(error) if error.kind() == ErrorKind::NotFound => {
      log::info!("clean destination, moving on");
    }
    unknown => log::warn!("unknown destination check - {unknown:?}"),
  }

  let asset = match latest.assets.into_iter().find(|item| {
    let match_prefix = item.name.starts_with(&config.artifact_naming.starts_with);
    let match_suffix = item.name.ends_with(&config.artifact_naming.ends_with);
    match_prefix && match_suffix
  }) {
    Some(inner) => inner,
    None => return Ok(()),
  };

  log::info!("found artifact match {} ('{}' @ '{}')", asset.id, asset.name, asset.url);

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

  log::info!("locate: {:?}", locate_response.status());

  let real_location = locate_response
    .header("Location")
    .and_then(|value| value.get(0))
    .map(|value| value.as_str().to_string())
    .ok_or_else(|| Error::new(ErrorKind::Other, "bad response from github"))?;

  log::info!("real location: {real_location}");

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

  let bytes = download_response.body_bytes().await.map_err(|error| {
    log::warn!("{error}");
    Error::new(ErrorKind::Other, "failed-download")
  })?;

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

  async_std::fs::rename(&temp_dir, &config.extraction.destination).await?;

  log::info!("success, writing new version to storage");
  let mut file = async_std::fs::File::create(&config.semver_storage).await?;
  async_std::write!(&mut file, "{}", latest.name).await?;

  Ok(())
}

async fn run(mut config: UpdaterConfig, mut flags: InitialRunFlags) -> Result<()> {
  let mut interval = async_std::stream::interval(std::time::Duration::from_secs(1));

  log::info!("entering working loop for config: {config:?}");

  loop {
    interval.next().await;

    let units = config
      .units
      .unwrap_or_default()
      .drain(0..)
      .collect::<Vec<UpdaterUnitConfig>>();

    let mut next = Vec::with_capacity(units.len());

    for unit in units {
      let result = match unit {
        UpdaterUnitConfig::GithubRelease(ref config) => github_release(&config, &flags).await,
      };

      if let Err(error) = result {
        log::warn!("updater failed on unit - {unit:?} - {error}");
        continue;
      }

      next.push(unit);
    }

    flags = InitialRunFlags::default();

    if next.len() == 0 {
      return Err(Error::new(ErrorKind::Other, "no units left"));
    }

    config.units = Some(next);
  }
}

fn main() -> Result<()> {
  let _ = dotenv::dotenv();
  env_logger::init();
  log::info!("env loaded");

  let options = UpdaterCommandLineOptions::parse();
  let ex = async_executor::LocalExecutor::new();
  let config_content = std::fs::read(&options.config)?;
  let config = toml::from_slice::<UpdaterConfig>(&config_content)?;

  let mut flags = InitialRunFlags::default();

  if options.run_immediately {
    flags = InitialRunFlags::Update;
  }

  futures_lite::future::block_on(ex.run(run(config, flags)))
}
