//! Local development example for Env-Secret-Hydrator.

use esh::{
    DotenvProvider, EnvProvider, Fallback, Hydrator, MemoryAuditor, MemoryCache, SecretString,
    Validator,
};
use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Deserialize)]
struct AppConfig {
    database_url: SecretString,
    #[serde(default = "default_region")]
    region: String,
}

fn default_region() -> String {
    "local".into()
}

#[tokio::main]
async fn main() -> esh::Result<()> {
    let auditor = MemoryAuditor::new();

    let hydrator = Hydrator::builder()
        .provider(EnvProvider::new())
        .provider(DotenvProvider::from_path(".env.example").or_else(|_| DotenvProvider::from_default())?)
        .with_fallback(Fallback::Local)
        .cache(MemoryCache::ttl(Duration::from_secs(60)))
        .on_audit(auditor.clone())
        .keys(vec!["database_url".into(), "region".into()])
        .validator(Validator::new().require("database_url"))
        .defaults({
            let mut m = esh::ConfigMap::new();
            m.insert(
                "database_url".into(),
                esh::ConfigValue::secret("postgres://localhost/app"),
            );
            m.insert("region".into(), esh::ConfigValue::string("local"));
            m
        })
        .build()
        .await?;

    let config = hydrator.load::<AppConfig>().await?;
    println!("loaded region={}", config.region);
    println!("database_url={:?}", config.database_url);
    println!("audit_events={}", auditor.events().len());
    Ok(())
}
