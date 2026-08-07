//! Temp filesystem helpers for dotenv / Docker / K8s secret fixtures.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static TEMP_SEQ: AtomicU64 = AtomicU64::new(0);

/// Unique temp directory holding secret files.
pub struct TempSecrets {
    pub root: PathBuf,
}

impl TempSecrets {
    pub fn create() -> Self {
        let seq = TEMP_SEQ.fetch_add(1, Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!(
            "esh-test-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            seq
        ));
        fs::create_dir_all(&root).expect("create temp secrets dir");
        Self { root }
    }

    pub fn write(&self, name: &str, contents: &str) -> PathBuf {
        let path = self.root.join(name);
        let mut f = fs::File::create(&path).expect("create secret file");
        write!(f, "{contents}").expect("write secret file");
        path
    }

    pub fn path(&self) -> &Path {
        &self.root
    }
}

impl Drop for TempSecrets {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// Write a temporary `.env` file and return its path (parent cleaned by caller or `TempSecrets`).
pub fn write_temp_dotenv(contents: &str) -> (TempSecrets, PathBuf) {
    let tmp = TempSecrets::create();
    let path = tmp.write(".env", contents);
    (tmp, path)
}

/// Write a directory of key→value secret files (Docker / K8s style).
pub fn write_secret_dir(entries: &[(&str, &str)]) -> TempSecrets {
    let tmp = TempSecrets::create();
    for (k, v) in entries {
        tmp.write(k, v);
    }
    tmp
}
