//! Encrypted local password vault.
//!
//! The master key is derived with Argon2id and kept only in memory while unlocked.
//! Individual credential fields are encrypted with XChaCha20-Poly1305.

use std::{
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use argon2::{Algorithm, Argon2, Params, Version};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    XChaCha20Poly1305, XNonce,
};
use rand::RngCore;
use zeroize::{Zeroize, Zeroizing};

use crate::storage::{BrowserStorage, EncryptedVaultRow};

const VERIFIER_PLAINTEXT: &[u8] = b"VERTEX_VAULT_VERIFIED_V1";
const KDF_PARAMS: &str = "argon2id:m=65536,t=3,p=1;xchacha20poly1305";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VaultStatus {
    pub is_configured: bool,
    pub is_unlocked: bool,
    pub entry_count: usize,
}

#[derive(Clone, PartialEq, Eq)]
pub struct PasswordCredential {
    pub id: String,
    pub origin: String,
    pub title: String,
    pub username: String,
    pub password: String,
    pub notes: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl Drop for PasswordCredential {
    fn drop(&mut self) {
        self.username.zeroize();
        self.password.zeroize();
        if let Some(notes) = &mut self.notes {
            notes.zeroize();
        }
    }
}

pub struct PasswordVault {
    master_key: Mutex<Option<Zeroizing<[u8; 32]>>>,
}

impl Default for PasswordVault {
    fn default() -> Self {
        Self::new()
    }
}

impl PasswordVault {
    pub fn new() -> Self {
        Self {
            master_key: Mutex::new(None),
        }
    }

    pub fn status(&self, storage: &BrowserStorage) -> Result<VaultStatus, String> {
        let is_configured = storage
            .get_vault_meta()
            .map_err(|error| error.to_string())?
            .is_some();
        let is_unlocked = self
            .master_key
            .lock()
            .map_err(|_| "Сховище паролів заблоковане через помилку синхронізації".to_owned())?
            .is_some();
        let entry_count = storage
            .list_password_rows()
            .map_err(|error| error.to_string())?
            .len();
        Ok(VaultStatus {
            is_configured,
            is_unlocked,
            entry_count,
        })
    }

    fn derive_key(master_password: &str, salt: &[u8]) -> Result<Zeroizing<[u8; 32]>, String> {
        let params = Params::new(65536, 3, 1, Some(32))
            .map_err(|error| error.to_string())?;
        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
        let mut key = Zeroizing::new([0u8; 32]);
        argon2
            .hash_password_into(master_password.as_bytes(), salt, &mut *key)
            .map_err(|error| error.to_string())?;
        Ok(key)
    }

    fn encrypt_bytes(key: &[u8; 32], plaintext: &[u8]) -> Result<String, String> {
        let cipher = XChaCha20Poly1305::new(key.into());
        let mut nonce_bytes = [0u8; 24];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = XNonce::from_slice(&nonce_bytes);
        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|_| "Не вдалося зашифрувати дані сховища".to_owned())?;
        let mut combined = Vec::with_capacity(nonce_bytes.len() + ciphertext.len());
        combined.extend_from_slice(&nonce_bytes);
        combined.extend_from_slice(&ciphertext);
        Ok(BASE64.encode(combined))
    }

    fn decrypt_bytes(key: &[u8; 32], encoded: &str) -> Result<Vec<u8>, String> {
        let combined = BASE64
            .decode(encoded)
            .map_err(|_| "Пошкоджені зашифровані дані сховища".to_owned())?;
        if combined.len() < 24 {
            return Err("Пошкоджені зашифровані дані сховища".to_owned());
        }
        let (nonce_bytes, ciphertext) = combined.split_at(24);
        let cipher = XChaCha20Poly1305::new(key.into());
        cipher
            .decrypt(XNonce::from_slice(nonce_bytes), ciphertext)
            .map_err(|_| "Неправильний майстер-пароль або пошкоджений запис".to_owned())
    }

    fn require_key(&self) -> Result<Zeroizing<[u8; 32]>, String> {
        self.master_key
            .lock()
            .map_err(|_| "Сховище паролів заблоковане через помилку синхронізації".to_owned())?
            .clone()
            .ok_or_else(|| "Спочатку розблокуй сховище паролів".to_owned())
    }

    pub fn setup_master_password(
        &self,
        storage: &BrowserStorage,
        master_password: &str,
    ) -> Result<(), String> {
        if master_password.chars().count() < 8 {
            return Err("Майстер-пароль має містити щонайменше 8 символів".to_owned());
        }
        if storage.get_vault_meta().map_err(|error| error.to_string())?.is_some() {
            return Err("Сховище вже налаштоване. Використай розблокування.".to_owned());
        }

        let mut salt = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut salt);
        let key = Self::derive_key(master_password, &salt)?;
        let verifier = Self::encrypt_bytes(&key, VERIFIER_PLAINTEXT)?;
        storage
            .save_vault_meta(&BASE64.encode(salt), &verifier, KDF_PARAMS)
            .map_err(|error| error.to_string())?;
        *self.master_key
            .lock()
            .map_err(|_| "Не вдалося отримати доступ до сховища паролів".to_owned())? =
            Some(key);
        Ok(())
    }

    pub fn unlock(&self, storage: &BrowserStorage, master_password: &str) -> Result<(), String> {
        let Some((salt_b64, verifier_enc, _)) =
            storage.get_vault_meta().map_err(|error| error.to_string())?
        else {
            return Err("Сховище паролів ще не налаштоване".to_owned());
        };
        let salt = BASE64
            .decode(salt_b64)
            .map_err(|_| "Пошкоджені параметри сховища паролів".to_owned())?;
        let key = Self::derive_key(master_password, &salt)?;
        let verifier = Self::decrypt_bytes(&key, &verifier_enc)
            .map_err(|_| "Неправильний майстер-пароль".to_owned())?;
        if verifier.as_slice() != VERIFIER_PLAINTEXT {
            return Err("Неправильний майстер-пароль".to_owned());
        }
        *self.master_key
            .lock()
            .map_err(|_| "Не вдалося отримати доступ до сховища паролів".to_owned())? =
            Some(key);
        Ok(())
    }

    pub fn lock(&self) {
        if let Ok(mut key) = self.master_key.lock() {
            *key = None;
        }
    }

    pub fn save_credential(
        &self,
        storage: &BrowserStorage,
        origin: &str,
        title: &str,
        username: &str,
        password: &str,
        notes: Option<&str>,
    ) -> Result<String, String> {
        let key = self.require_key()?;
        if password.is_empty() {
            return Err("Пароль не може бути порожнім".to_owned());
        }
        let normalized_origin = normalize_origin(origin)?;
        let title = if title.trim().is_empty() {
            normalized_origin.clone()
        } else {
            title.trim().to_owned()
        };
        let now = unix_time_ms();
        let id = uuid::Uuid::new_v4().to_string();
        let username_enc = Self::encrypt_bytes(&key, username.as_bytes())?;
        let password_enc = Self::encrypt_bytes(&key, password.as_bytes())?;
        let notes = notes.filter(|value| !value.is_empty());
        let notes_enc = notes
            .map(|value| Self::encrypt_bytes(&key, value.as_bytes()))
            .transpose()?;

        let row = EncryptedVaultRow {
            id: id.clone(),
            origin: normalized_origin.clone(),
            title: title.clone(),
            username_enc,
            password_enc,
            notes_enc,
            created_at: now,
            updated_at: now,
        };
        storage
            .upsert_password_row(&row)
            .map_err(|error| error.to_string())?;

        Ok(id)
    }

    pub fn list_credentials(
        &self,
        storage: &BrowserStorage,
        search: Option<&str>,
    ) -> Result<Vec<PasswordCredential>, String> {
        let key = self.require_key()?;
        let rows = storage
            .list_password_rows()
            .map_err(|error| error.to_string())?;
        let needle = search.unwrap_or_default().trim().to_lowercase();
        let mut credentials = Vec::with_capacity(rows.len());

        for row in rows {
            let username = String::from_utf8(Self::decrypt_bytes(&key, &row.username_enc)?)
                .map_err(|_| "Некоректне кодування імені користувача".to_owned())?;
            let password = String::from_utf8(Self::decrypt_bytes(&key, &row.password_enc)?)
                .map_err(|_| "Некоректне кодування пароля".to_owned())?;
            let notes = row
                .notes_enc
                .as_deref()
                .map(|value| Self::decrypt_bytes(&key, value))
                .transpose()?
                .map(String::from_utf8)
                .transpose()
                .map_err(|_| "Некоректне кодування нотаток".to_owned())?;

            if !needle.is_empty()
                && !row.origin.to_lowercase().contains(&needle)
                && !row.title.to_lowercase().contains(&needle)
                && !username.to_lowercase().contains(&needle)
            {
                continue;
            }

            credentials.push(PasswordCredential {
                id: row.id,
                origin: row.origin,
                title: row.title,
                username,
                password,
                notes,
                created_at: row.created_at,
                updated_at: row.updated_at,
            });
        }
        Ok(credentials)
    }

    pub fn find_for_origin(
        &self,
        storage: &BrowserStorage,
        page_url: &str,
    ) -> Result<Vec<PasswordCredential>, String> {
        let target = normalize_origin(page_url)?;
        Ok(self
            .list_credentials(storage, None)?
            .into_iter()
            .filter(|credential| credential.origin == target)
            .collect())
    }

    pub fn delete_credential(&self, storage: &BrowserStorage, id: &str) -> Result<(), String> {
        let _ = self.require_key()?;
        storage
            .delete_password_row(id)
            .map_err(|error| error.to_string())
    }
}

pub fn normalize_origin(raw: &str) -> Result<String, String> {
    let parsed = url::Url::parse(raw.trim())
        .map_err(|_| "Введи коректну адресу сайту з http:// або https://".to_owned())?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("Паролі можна зберігати лише для HTTP- та HTTPS-сайтів".to_owned());
    }
    Ok(parsed.origin().ascii_serialization())
}

fn unix_time_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn creates_unlocks_and_locks_vault() {
        let storage = BrowserStorage::open(Path::new(":memory:")).unwrap();
        let vault = PasswordVault::new();
        assert!(!vault.status(&storage).unwrap().is_configured);
        assert!(vault.setup_master_password(&storage, "short").is_err());
        vault.setup_master_password(&storage, "correct horse battery").unwrap();
        assert!(vault.status(&storage).unwrap().is_unlocked);
        vault.lock();
        assert!(!vault.status(&storage).unwrap().is_unlocked);
        assert!(vault.unlock(&storage, "incorrect password").is_err());
        vault.unlock(&storage, "correct horse battery").unwrap();
    }

    #[test]
    fn encrypts_lists_and_deletes_credentials() {
        let storage = BrowserStorage::open(Path::new(":memory:")).unwrap();
        let vault = PasswordVault::new();
        vault.setup_master_password(&storage, "correct horse battery").unwrap();
        let saved_id = vault.save_credential(
            &storage,
            "https://www.example.com/login?next=1",
            "Example",
            "alice",
            "secret-value",
            Some("test note"),
        ).unwrap();
        assert_eq!(saved.origin, "https://www.example.com");
        let found = vault.find_for_origin(&storage, "https://www.example.com/other").unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].password, "secret-value");
        vault.delete_credential(&storage, &saved_id).unwrap();
        assert!(vault.list_credentials(&storage, None).unwrap().is_empty());
    }

    #[test]
    fn origin_matching_does_not_accept_lookalike_hosts() {
        assert_eq!(
            normalize_origin("https://example.com/path").unwrap(),
            "https://example.com"
        );
        assert_ne!(
            normalize_origin("https://example.com").unwrap(),
            normalize_origin("https://notexample.com").unwrap()
        );
    }
}
