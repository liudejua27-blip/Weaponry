use std::io;

use serde_json::Value;

use thiserror::Error;

pub type CoreResult<T> = Result<T, CoreError>;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("SQLite operation failed")]
    Sqlite(#[from] rusqlite::Error),
    #[error("filesystem operation failed")]
    Io(#[from] io::Error),
    #[error("{code}: {message}")]
    InvalidData { code: &'static str, message: String },
    #[error("{code}: {message}")]
    Conflict { code: &'static str, message: String },
    #[error("{code}: {message}")]
    ConflictWithDetails {
        code: &'static str,
        message: String,
        details: Value,
    },
    #[error("{kind} was not found")]
    NotFound { kind: &'static str },
    #[error("SQLite migration {version} failed")]
    Migration {
        version: String,
        #[source]
        source: Box<CoreError>,
    },
}

impl CoreError {
    pub fn invalid_data(code: &'static str, message: impl Into<String>) -> Self {
        Self::InvalidData {
            code,
            message: message.into(),
        }
    }

    pub fn conflict(code: &'static str, message: impl Into<String>) -> Self {
        Self::Conflict {
            code,
            message: message.into(),
        }
    }

    pub fn conflict_with_details(
        code: &'static str,
        message: impl Into<String>,
        details: Value,
    ) -> Self {
        Self::ConflictWithDetails {
            code,
            message: message.into(),
            details,
        }
    }

    pub fn not_found(kind: &'static str) -> Self {
        Self::NotFound { kind }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::Sqlite(_) => "SQLITE_OPERATION_FAILED",
            Self::Io(_) => "FILESYSTEM_OPERATION_FAILED",
            Self::InvalidData { code, .. }
            | Self::Conflict { code, .. }
            | Self::ConflictWithDetails { code, .. } => code,
            Self::NotFound { .. } => "RESOURCE_NOT_FOUND",
            Self::Migration { .. } => "SQLITE_MIGRATION_FAILED",
        }
    }
}
