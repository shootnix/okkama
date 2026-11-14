//! Выбор backend и генерация выходного артефакта из IR.
//!
//! Каждый backend располагается в своём подмодуле. Общая точка входа
//! [`generate`] проверяет модуль, направляет его выбранному генератору и
//! возвращает [`Artifact`] (основной файл и вспомогательные) либо ошибку.
//! Запись файлов и параметры командной строки остаются в CLI.

pub mod sparrow;

use std::fmt;

use crate::ir;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Backend {
    Sparrow,
}

impl Backend {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "sparrow" => Some(Self::Sparrow),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Sparrow => "sparrow",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CodegenError {
    InvalidInput { backend: Backend, message: String },
}

impl fmt::Display for CodegenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput { backend, message } => {
                write!(f, "backend `{}`: {message}", backend.name())
            }
        }
    }
}

impl std::error::Error for CodegenError {}

/// Логическое имя выходного артефакта, без каталога и расширения.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub artifact_name: String,
}

/// Один выходной файл backend: имя относительно каталога основного артефакта.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtifactFile {
    pub name: String,
    pub bytes: Vec<u8>,
}

/// Результат генерации: основной файл первым, затем вспомогательные.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Artifact {
    pub files: Vec<ArtifactFile>,
}

impl Artifact {
    /// Байты основного файла.
    pub fn main(&self) -> &[u8] {
        &self.files[0].bytes
    }
}

pub fn generate(
    module: &ir::Module,
    backend: Backend,
    options: &Options,
) -> Result<Artifact, CodegenError> {
    let files = match backend {
        Backend::Sparrow => {
            let program =
                sparrow::generate(module).map_err(|error| CodegenError::InvalidInput {
                    backend,
                    message: error.to_string(),
                })?;
            let bytes =
                okkama_sparrow::encode(&program).map_err(|error| CodegenError::InvalidInput {
                    backend,
                    message: error.to_string(),
                })?;
            vec![(format!("{}.spb", options.artifact_name), bytes)]
        }
    };
    Ok(Artifact {
        files: files
            .into_iter()
            .map(|(name, bytes)| ArtifactFile { name, bytes })
            .collect(),
    })
}
