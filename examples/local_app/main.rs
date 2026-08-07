//! Local development example for Env-Secret-Hydrator.

use env_secret_hydrator::{
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
async fn main() -> env_secret_hydrator::Result<()> {
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
            let mut m = env_secret_hydrator::ConfigMap::new();
            m.insert(
                "database_url".into(),
                env_secret_hydrator::ConfigValue::secret("postgres://localhost/app"),
            );
            m.insert("region".into(), env_secret_hydrator::ConfigValue::string("local"));
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
