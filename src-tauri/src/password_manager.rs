use crate::db::{Database, EncryptedVaultRow};
use crate::error::{VortexError, VortexResult};
use argon2::{Algorithm, Argon2, Params, Version};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    XChaCha20Poly1305, XNonce,
};
use chrono::Utc;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use zeroize::Zeroizing;

const VERIFIER_PLAINTEXT: &[u8] = b"VORTEX_VAULT_VERIFIED_V1";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultStatus {
    pub is_configured: bool,
    pub is_unlocked: bool,
    pub entry_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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

#[derive(Debug, Deserialize)]
struct GoogleCsvRow {
    name: Option<String>,
    url: String,
    username: String,
    password: String,
    note: Option<String>,
}

pub struct PasswordManager {
    db: Arc<Database>,
    // Головний 256-бітний ключ зберігається ТІЛЬКИ в RAM і затирається при блокуванні/виході (Zeroize)
    master_key: Mutex<Option<Zeroizing<[u8; 32]>>>,
}

impl PasswordManager {
    pub fn new(db: Arc<Database>) -> Self {
        Self {
            db,
            master_key: Mutex::new(None),
        }
    }

    pub fn status(&self) -> VortexResult<VaultStatus> {
        let is_configured = self.db.get_vault_meta()?.is_some();
        let is_unlocked = self.master_key.lock().unwrap().is_some();
        let entry_count = self.db.list_password_rows()?.len();
        Ok(VaultStatus {
            is_configured,
            is_unlocked,
            entry_count,
        })
    }

    /// Отримання ключа з майстер-пароля за допомогою Argon2id (64 MiB, 3 ітерації, 1 потік)
    fn derive_key(master_password: &str, salt: &[u8]) -> VortexResult<Zeroizing<[u8; 32]>> {
        let params = Params::new(65536, 3, 1, Some(32))
            .map_err(|e| VortexError::Crypto(e.to_string()))?;
        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
        let mut key = Zeroizing::new([0u8; 32]);
        argon2
            .hash_password_into(master_password.as_bytes(), salt, &mut *key)
            .map_err(|e| VortexError::Crypto(e.to_string()))?;
        Ok(key)
    }

    fn encrypt_bytes(key: &[u8; 32], plaintext: &[u8]) -> VortexResult<String> {
        let cipher = XChaCha20Poly1305::new(key.into());
        let mut nonce_bytes = [0u8; 24];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = XNonce::from_slice(&nonce_bytes);
        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| VortexError::Crypto(e.to_string()))?;
        let mut combined = Vec::with_capacity(24 + ciphertext.len());
        combined.extend_from_slice(&nonce_bytes);
        combined.extend_from_slice(&ciphertext);
        Ok(B64.encode(combined))
    }

    fn decrypt_bytes(key: &[u8; 32], encoded: &str) -> VortexResult<Vec<u8>> {
        let combined = B64
            .decode(encoded)
            .map_err(|e| VortexError::Crypto(e.to_string()))?;
        if combined.len() < 24 {
            return Err(VortexError::Crypto("Пошкоджений шифротекст".into()));
        }
        let (nonce_bytes, ciphertext) = combined.split_at(24);
        let cipher = XChaCha20Poly1305::new(key.into());
        let nonce = XNonce::from_slice(nonce_bytes);
        cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| VortexError::InvalidMasterPassword)
    }

    pub fn setup_master_password(&self, master_password: &str) -> VortexResult<()> {
        if master_password.chars().count() < 8 {
            return Err(VortexError::Validation(
                "Майстер-пароль має містити щонайменше 8 символів".into(),
            ));
        }
        let mut salt = [0u8; 16];
        rand::thread_rng().fill_bytes(&mut salt);
        let key = Self::derive_key(master_password, &salt)?;
        let verifier_enc = Self::encrypt_bytes(&key, VERIFIER_PLAINTEXT)?;

        self.db.save_vault_meta(
            &B64.encode(salt),
            &verifier_enc,
            "argon2id:m=65536,t=3,p=1;xchacha20poly1305",
        )?;
        *self.master_key.lock().unwrap() = Some(key);
        Ok(())
    }

    pub fn unlock(&self, master_password: &str) -> VortexResult<()> {
        let Some((salt_b64, verifier_enc, _)) = self.db.get_vault_meta()? else {
            return Err(VortexError::VaultLocked);
        };
        let salt = B64
            .decode(salt_b64)
            .map_err(|e| VortexError::Crypto(e.to_string()))?;
        let key = Self::derive_key(master_password, &salt)?;
        let decrypted = Self::decrypt_bytes(&key, &verifier_enc)?;
        if decrypted.as_slice() != VERIFIER_PLAINTEXT {
            return Err(VortexError::InvalidMasterPassword);
        }
        *self.master_key.lock().unwrap() = Some(key);
        Ok(())
    }

    pub fn lock(&self) {
        *self.master_key.lock().unwrap() = None;
    }

    fn require_key(&self) -> VortexResult<Zeroizing<[u8; 32]>> {
        self.master_key
            .lock()
            .unwrap()
            .clone()
            .ok_or(VortexError::VaultLocked)
    }

    pub fn save_credential(
        &self,
        id: Option<String>,
        origin: String,
        title: String,
        username: String,
        password: String,
        notes: Option<String>,
    ) -> VortexResult<PasswordCredential> {
        let key = self.require_key()?;
        let now = Utc::now().timestamp_millis();
        let entry_id = id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let normalized_origin = normalize_origin(&origin);

        let username_enc = Self::encrypt_bytes(&key, username.as_bytes())?;
        let password_enc = Self::encrypt_bytes(&key, password.as_bytes())?;
        let notes_enc = match &notes {
            Some(n) if !n.is_empty() => Some(Self::encrypt_bytes(&key, n.as_bytes())?),
            _ => None,
        };

        let row = EncryptedVaultRow {
            id: entry_id.clone(),
            origin: normalized_origin.clone(),
            title: title.clone(),
            username_enc,
            password_enc,
            notes_enc,
            created_at: now,
            updated_at: now,
        };
        self.db.upsert_password_row(&row)?;

        Ok(PasswordCredential {
            id: entry_id,
            origin: normalized_origin,
            title,
            username,
            password,
            notes,
            created_at: now,
            updated_at: now,
        })
    }

    pub fn list_credentials(&self, search: Option<&str>) -> VortexResult<Vec<PasswordCredential>> {
        let key = self.require_key()?;
        let rows = self.db.list_password_rows()?;
        let needle = search.unwrap_or("").trim().to_lowercase();
        let mut out = Vec::with_capacity(rows.len());

        for r in rows {
            let username = String::from_utf8(Self::decrypt_bytes(&key, &r.username_enc)?)
                .map_err(|e| VortexError::Crypto(e.to_string()))?;
            let password = String::from_utf8(Self::decrypt_bytes(&key, &r.password_enc)?)
                .map_err(|e| VortexError::Crypto(e.to_string()))?;
            let notes = match r.notes_enc {
                Some(ref enc) => Some(
                    String::from_utf8(Self::decrypt_bytes(&key, enc)?)
                        .map_err(|e| VortexError::Crypto(e.to_string()))?,
                ),
                None => None,
            };

            if !needle.is_empty()
                && !r.origin.to_lowercase().contains(&needle)
                && !r.title.to_lowercase().contains(&needle)
                && !username.to_lowercase().contains(&needle)
            {
                continue;
            }

            out.push(PasswordCredential {
                id: r.id,
                origin: r.origin,
                title: r.title,
                username,
                password,
                notes,
                created_at: r.created_at,
                updated_at: r.updated_at,
            });
        }

        Ok(out)
    }

    pub fn find_for_origin(&self, page_url: &str) -> VortexResult<Vec<PasswordCredential>> {
        let target = normalize_origin(page_url);
        let all = self.list_credentials(None)?;
        Ok(all
            .into_iter()
            .filter(|c| c.origin == target || target.ends_with(&c.origin))
            .collect())
    }

    pub fn delete_credential(&self, id: &str) -> VortexResult<()> {
        let _ = self.require_key()?;
        self.db.delete_password_row(id)
    }

    /// Імпорт паролів із CSV-файлу Google Password Manager (`name,url,username,password,note`).
    pub fn import_google_csv(&self, csv_content: &str) -> VortexResult<usize> {
        let _ = self.require_key()?;
        let mut reader = csv::ReaderBuilder::new()
            .flexible(true)
            .from_reader(csv_content.as_bytes());
        let mut imported = 0usize;

        for result in reader.deserialize::<GoogleCsvRow>() {
            if let Ok(row) = result {
                if row.url.trim().is_empty() || row.password.is_empty() {
                    continue;
                }
                let origin = normalize_origin(&row.url);
                let title = row
                    .name
                    .filter(|n| !n.trim().is_empty())
                    .unwrap_or_else(|| origin.clone());
                self.save_credential(None, origin, title, row.username, row.password, row.note)?;
                imported += 1;
            }
        }
        Ok(imported)
    }
}

fn normalize_origin(raw: &str) -> String {
    if let Ok(url) = url::Url::parse(raw.trim()) {
        if let Some(host) = url.host_str() {
            return host.to_lowercase();
        }
    }
    raw.trim().to_lowercase()
}
