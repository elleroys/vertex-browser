use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum VortexError {
    #[error("Помилка бази даних SQLite: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Помилка мережі: {0}")]
    Network(#[from] reqwest::Error),

    #[error("Помилка вводу/виводу: {0}")]
    Io(#[from] std::io::Error),

    #[error("Некоректний URL: {0}")]
    InvalidUrl(String),

    #[error("Сховище паролів заблоковане або не налаштоване")]
    VaultLocked,

    #[error("Невірний майстер-пароль")]
    InvalidMasterPassword,

    #[error("Криптографічна помилка: {0}")]
    Crypto(String),

    #[error("Вкладка не знайдена: {0}")]
    TabNotFound(String),

    #[error("Помилка WebView2 / Tauri: {0}")]
    Webview(String),

    #[error("Неприпустимі вхідні дані: {0}")]
    Validation(String),
}

#[derive(Serialize)]
struct SerializedError {
    code: &'static str,
    message: String,
}

impl Serialize for VortexError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let code = match self {
            VortexError::Database(_) => "DB_ERROR",
            VortexError::Network(_) => "NETWORK_ERROR",
            VortexError::Io(_) => "IO_ERROR",
            VortexError::InvalidUrl(_) => "INVALID_URL",
            VortexError::VaultLocked => "VAULT_LOCKED",
            VortexError::InvalidMasterPassword => "INVALID_MASTER_PASSWORD",
            VortexError::Crypto(_) => "CRYPTO_ERROR",
            VortexError::TabNotFound(_) => "TAB_NOT_FOUND",
            VortexError::Webview(_) => "WEBVIEW_ERROR",
            VortexError::Validation(_) => "VALIDATION_ERROR",
        };
        SerializedError {
            code,
            message: self.to_string(),
        }
        .serialize(serializer)
    }
}

pub type VortexResult<T> = Result<T, VortexError>;
