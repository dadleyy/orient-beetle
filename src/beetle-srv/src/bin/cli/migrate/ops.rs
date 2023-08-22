use anyhow::Context;
use async_std::stream::StreamExt;

pub(super) async fn from_to<S, F, O, T>(
  config: &crate::cli::CommandLineConfig,
  collection: S,
  mapper: F,
) -> anyhow::Result<()>
where
  S: AsRef<str>,
  F: Fn(O) -> (T, bson::Document),
  O: for<'a> serde::Deserialize<'a> + serde::Serialize + std::fmt::Debug,
  T: for<'a> serde::Deserialize<'a> + serde::Serialize + std::fmt::Debug,
{
  let mongo = beetle::mongo::connect_mongo(&config.mongo).await?;
  let db = mongo.database(&config.mongo.database);
  let origin_collection = db.collection::<O>(collection.as_ref());
  let target_collection = db.collection::<T>(collection.as_ref());

  log::info!("====== RUNNING migration for '{}'", collection.as_ref());

  let mut cursor = origin_collection
    .find(bson::doc! { "device_id": { "$exists": 1 } }, None)
    .await?;

  let mut updates = vec![];
  while let Some(n) = cursor.next().await {
    log::debug!("attemping to migrate '{n:?}' in collection '{}'", collection.as_ref());
    let record = n.with_context(|| "unable to deserialize into old")?;
    updates.push(mapper(record));
  }

  log::info!("applying {} update(s)", updates.len());
  for (update, query) in updates {
    log::debug!("applying update '{update:?}'");
    target_collection
      .find_one_and_replace(
        query,
        update,
        mongodb::options::FindOneAndReplaceOptions::builder()
          .return_document(mongodb::options::ReturnDocument::After)
          .build(),
      )
      .await?;
  }

  log::info!("====== COMPLETE migration for '{}'", collection.as_ref());

  Ok(())
}
