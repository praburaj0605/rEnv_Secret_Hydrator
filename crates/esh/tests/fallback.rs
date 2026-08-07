//! Fallback chain integration tests (FR-005).

use esh::{
    AwsSecretsManagerProvider, ConfigValue, DotenvProvider, EnvProvider, Fallback, Hydrator,
    MapAwsClient, SecretString,
};
use esh_test_support::{write_temp_dotenv, SAMPLE_SECRET};
use serde::Deserialize;
use std::sync::{Arc, OnceLock};
use tokio::sync::Mutex;

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[derive(Debug, Deserialize)]
struct Cfg {
    database_url: SecretString,
    #[serde(default)]
    region: String,
}

#[tokio::test]
async fn cloud_miss_falls_through_to_env_then_dotenv() {
    let _g = env_lock().lock().await;
    std::env::remove_var("database_url");
    let (_tmp, path) = write_temp_dotenv(&format!("database_url={SAMPLE_SECRET}\nregion=eu\n"));
    std::env::set_var("region", "from-env");

    let aws = AwsSecretsManagerProvider::new(Arc::new(MapAwsClient::new()));
    let cfg = Hydrator::builder()
        .provider(aws)
        .provider(EnvProvider::new())
        .provider(DotenvProvider::from_path(&path).unwrap())
        .with_fallback(Fallback::Local)
        .keys(vec!["database_url".into(), "region".into()])
        .build()
        .await
        .unwrap()
        .load::<Cfg>()
        .await
        .unwrap();

    assert_eq!(cfg.database_url.expose(), SAMPLE_SECRET);
    assert_eq!(cfg.region, "from-env");
    std::env::remove_var("region");
}

#[tokio::test]
async fn defaults_when_all_providers_miss() {
    let _g = env_lock().lock().await;
    std::env::remove_var("database_url");
    std::env::remove_var("region");

    let mut defaults = esh::ConfigMap::new();
    defaults.insert(
        "database_url".into(),
        ConfigValue::secret("postgres://default"),
    );
    defaults.insert("region".into(), ConfigValue::string("local"));

    let cfg = Hydrator::builder()
        .provider(EnvProvider::new())
        .keys(vec!["database_url".into(), "region".into()])
        .defaults(defaults)
        .build()
        .await
        .unwrap()
        .load::<Cfg>()
        .await
        .unwrap();

    assert_eq!(cfg.database_url.expose(), "postgres://default");
    assert_eq!(cfg.region, "local");
}
