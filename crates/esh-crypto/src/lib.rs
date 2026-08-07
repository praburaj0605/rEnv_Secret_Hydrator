//! Encryption / decryption adapters (FR-011).

use async_trait::async_trait;
use esh_core::{Error, Result, SecretBytes, SecretString};

/// Decrypts ciphertext into plaintext secrets.
#[async_trait]
pub trait Decryptor: Send + Sync {
    /// Decrypt opaque ciphertext.
    async fn decrypt(&self, ciphertext: &[u8]) -> Result<SecretBytes>;
}

/// Encrypts plaintext (optional for libraries that only need decrypt-at-load).
#[async_trait]
pub trait Encryptor: Send + Sync {
    /// Encrypt plaintext bytes.
    async fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>>;
}

/// Cloud KMS decrypt interface (AWS KMS / Azure / GCP).
#[async_trait]
pub trait KmsClient: Send + Sync {
    /// Decrypt a ciphertext blob produced by the KMS key.
    async fn decrypt(&self, ciphertext: &[u8]) -> Result<SecretBytes>;
}

/// Adapter wrapping any [`KmsClient`] as a [`Decryptor`].
pub struct KmsDecryptor {
    client: std::sync::Arc<dyn KmsClient>,
}

impl KmsDecryptor {
    /// Create from a KMS client.
    #[must_use]
    pub fn new(client: std::sync::Arc<dyn KmsClient>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl Decryptor for KmsDecryptor {
    async fn decrypt(&self, ciphertext: &[u8]) -> Result<SecretBytes> {
        self.client.decrypt(ciphertext).await
    }
}

/// In-memory passthrough KMS fake for tests.
#[derive(Debug, Default, Clone, Copy)]
pub struct PassthroughKms;

#[async_trait]
impl KmsClient for PassthroughKms {
    async fn decrypt(&self, ciphertext: &[u8]) -> Result<SecretBytes> {
        Ok(SecretBytes::new(ciphertext.to_vec()))
    }
}

#[cfg(feature = "age")]
mod age_backend {
    use super::*;
    use age::x25519;
    use age::{Decryptor as AgeDecryptorApi, Encryptor as AgeEncryptorApi};
    use std::io::{Read, Write};

    /// Age identity-based decryptor.
    pub struct AgeIdentityDecryptor {
        identity: x25519::Identity,
    }

    impl AgeIdentityDecryptor {
        /// Parse an age secret identity string (`AGE-SECRET-KEY-...`).
        pub fn from_secret_string(secret: &str) -> Result<Self> {
            let identity: x25519::Identity = secret
                .trim()
                .parse()
                .map_err(|e| Error::Crypto(format!("invalid age identity: {e}")))?;
            Ok(Self { identity })
        }

        /// Generate a new identity (returns decryptor + public recipient string).
        #[must_use]
        pub fn generate() -> (Self, String) {
            let identity = x25519::Identity::generate();
            let recipient = identity.to_public().to_string();
            (Self { identity }, recipient)
        }
    }

    #[async_trait]
    impl Decryptor for AgeIdentityDecryptor {
        async fn decrypt(&self, ciphertext: &[u8]) -> Result<SecretBytes> {
            let decryptor = AgeDecryptorApi::new(ciphertext)
                .map_err(|e| Error::Crypto(format!("age decryptor: {e}")))?;
            let mut reader = match decryptor {
                AgeDecryptorApi::Recipients(d) => d
                    .decrypt(std::iter::once(&self.identity as &dyn age::Identity))
                    .map_err(|e| Error::Crypto(format!("age decrypt: {e}")))?,
                AgeDecryptorApi::Passphrase(_) => {
                    return Err(Error::Crypto(
                        "passphrase-encrypted age files are not supported by AgeIdentityDecryptor"
                            .into(),
                    ));
                }
            };
            let mut out = Vec::new();
            reader
                .read_to_end(&mut out)
                .map_err(|e| Error::Crypto(e.to_string()))?;
            Ok(SecretBytes::new(out))
        }
    }

    /// Encrypt to an age recipient (public key).
    pub fn age_encrypt(recipient: &str, plaintext: &[u8]) -> Result<Vec<u8>> {
        let recipient: x25519::Recipient = recipient
            .parse()
            .map_err(|e| Error::Crypto(format!("invalid age recipient: {e}")))?;
        let encryptor = AgeEncryptorApi::with_recipients(vec![Box::new(recipient) as Box<dyn age::Recipient + Send>])
            .ok_or_else(|| Error::Crypto("age encryptor requires a recipient".into()))?;
        let mut encrypted = vec![];
        {
            let mut writer = encryptor
                .wrap_output(&mut encrypted)
                .map_err(|e| Error::Crypto(e.to_string()))?;
            writer
                .write_all(plaintext)
                .map_err(|e| Error::Crypto(e.to_string()))?;
            writer
                .finish()
                .map_err(|e| Error::Crypto(e.to_string()))?;
        }
        Ok(encrypted)
    }

    /// Helper: decrypt age ciphertext to UTF-8 [`SecretString`].
    pub async fn decrypt_age_string(
        decryptor: &AgeIdentityDecryptor,
        ciphertext: &[u8],
    ) -> Result<SecretString> {
        let bytes = decryptor.decrypt(ciphertext).await?;
        let s = std::str::from_utf8(bytes.expose())
            .map_err(|e| Error::Crypto(format!("utf-8: {e}")))?
            .to_string();
        Ok(SecretString::new(s))
    }
}

#[cfg(feature = "age")]
pub use age_backend::{age_encrypt, decrypt_age_string, AgeIdentityDecryptor};

/// Marker / documentation helper for SOPS workflows.
///
/// SOPS files are typically decrypted out-of-band or via the `sops` CLI. This
/// type documents the integration point: feed decrypted YAML/JSON into the
/// hydrator defaults / dotenv path rather than embedding the SOPS binary.
#[derive(Debug, Default, Clone, Copy)]
pub struct SopsIntegration;

impl SopsIntegration {
    /// Reminder message for operators.
    #[must_use]
    pub fn guidance() -> &'static str {
        "Decrypt SOPS files with the sops CLI or native library bindings, then load plaintext via dotenv/defaults. Use AgeIdentityDecryptor or KmsDecryptor for envelope payloads embedded in config."
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn kms_passthrough_roundtrip() {
        let dec = KmsDecryptor::new(Arc::new(PassthroughKms));
        let pt = dec.decrypt(b"kms-secret-needle").await.unwrap();
        assert_eq!(pt.expose(), b"kms-secret-needle");
        assert!(!format!("{pt:?}").contains("kms-secret"));
    }

    #[test]
    fn sops_guidance_non_empty() {
        assert!(!SopsIntegration::guidance().is_empty());
    }

    #[cfg(feature = "age")]
    #[tokio::test]
    async fn age_roundtrip() {
        let (dec, recipient) = AgeIdentityDecryptor::generate();
        let ct = age_encrypt(&recipient, b"hello-secret").unwrap();
        let pt = dec.decrypt(&ct).await.unwrap();
        assert_eq!(pt.expose(), b"hello-secret");
        assert!(!format!("{pt:?}").contains("hello"));
    }

    #[cfg(feature = "age")]
    #[tokio::test]
    async fn age_decrypt_failure_on_garbage() {
        let (dec, _) = AgeIdentityDecryptor::generate();
        let err = dec.decrypt(b"not-age-ciphertext").await.unwrap_err();
        assert!(matches!(err, Error::Crypto(_)));
    }

    #[cfg(feature = "age")]
    #[test]
    fn age_invalid_identity() {
        match AgeIdentityDecryptor::from_secret_string("not-a-key") {
            Err(Error::Crypto(_)) => {}
            Ok(_) => panic!("expected crypto error"),
            Err(e) => panic!("unexpected error: {e}"),
        }
    }
}
