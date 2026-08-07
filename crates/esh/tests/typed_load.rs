//! Typed load tests (FR-006).

use esh::{DotenvProvider, EnvProvider, Fallback, Hydrator, SecretString, Validator};
use esh_test_support::{write_temp_dotenv, SAMPLE_SECRET};
use serde::Deserialize;
use std::sync::OnceLock;
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
async fn typed_load_positive() {
    let _g = env_lock().lock().await;
    let (_tmp, path) = write_temp_dotenv(&format!("database_url={SAMPLE_SECRET}\nregion=eu-west-1\n"));
    std::env::set_var("database_url", SAMPLE_SECRET);

    let config = Hydrator::builder()
        .provider(EnvProvider::new())
        .provider(DotenvProvider::from_path(&path).unwrap())
        .with_fallback(Fallback::Local)
        .keys(vec!["database_url".into(), "region".into()])
        .validator(Validator::new().require("database_url"))
        .build()
        .await
        .unwrap()
        .load::<Cfg>()
        .await
        .unwrap();

    assert_eq!(config.database_url.expose(), SAMPLE_SECRET);
    assert_eq!(config.region, "eu-west-1");
    std::env::remove_var("database_url");
}

#[tokio::test]
async fn typed_load_deserialize_failure() {
    let _g = env_lock().lock().await;
    std::env::set_var("database_url", SAMPLE_SECRET);
    let h = Hydrator::builder()
        .provider(EnvProvider::new())
        .keys(vec!["database_url".into()])
        .build()
        .await
        .unwrap();

    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    struct Bad {
        database_url: u64,
    }

    match h.load::<Bad>().await {
        Err(esh::Error::Deserialize(_)) => {}
        Err(e) => {
            assert!(!e.to_string().contains(SAMPLE_SECRET));
            panic!("unexpected: {e}");
        }
        Ok(_) => panic!("expected deserialize error"),
    }
    std::env::remove_var("database_url");
}
