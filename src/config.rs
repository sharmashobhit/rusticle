use fastembed;
use serde::Deserialize;
use std::fs;
use std::path::Path;
use toml;

#[derive(Deserialize, Debug)]
pub(crate) struct Config {
    #[serde(default)]
    pub(crate) server: Server,
    #[serde(default)]
    pub(crate) database: Database,
    #[serde(default)]
    pub(crate) embedding: Embedding,
}

#[derive(Deserialize, Debug)]
pub(crate) struct Server {
    #[serde(default = "default_host")]
    pub(crate) host: String,
    #[serde(default = "default_port")]
    pub(crate) port: u16,
}

#[derive(Deserialize, Debug)]
pub(crate) struct Database {
    #[serde(default = "default_db_path")]
    pub(crate) path: String,
}

#[derive(Deserialize, Debug)]
pub(crate) struct Embedding {
    #[serde(default = "default_model", deserialize_with = "deserialize_model")]
    pub(crate) model: fastembed::EmbeddingModel,
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_port() -> u16 {
    8080
}

fn default_db_path() -> String {
    "./data".to_string()
}

fn default_model() -> fastembed::EmbeddingModel {
    fastembed::EmbeddingModel::NomicEmbedTextV15Q
}

fn deserialize_model<'de, D>(deserializer: D) -> Result<fastembed::EmbeddingModel, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let model_str = String::deserialize(deserializer)?;
    match model_str.to_lowercase().as_str() {
        "all-minilm-l6-v2" => Ok(fastembed::EmbeddingModel::AllMiniLML6V2),
        "bge-small-en" => Ok(fastembed::EmbeddingModel::BGEBaseENV15),
        "bge-small-en-v1.5" => Ok(fastembed::EmbeddingModel::BGESmallENV15),
        "bge-base-en" => Ok(fastembed::EmbeddingModel::BGEBaseENV15),
        "nomic-embed-text-v1.5" => Ok(fastembed::EmbeddingModel::NomicEmbedTextV15),
        "nomic-embed-text-v1.5-q" => Ok(fastembed::EmbeddingModel::NomicEmbedTextV15Q),
        _ => Err(serde::de::Error::custom(format!(
            "Unknown model: {}",
            model_str
        ))),
    }
}

impl Default for Server {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
        }
    }
}

impl Default for Database {
    fn default() -> Self {
        Self {
            path: default_db_path(),
        }
    }
}

impl Default for Embedding {
    fn default() -> Self {
        Self {
            model: default_model(),
        }
    }
}

impl Config {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Self {
        fs::read_to_string(path)
            .ok()
            .and_then(|content| toml::from_str(&content).ok())
            .unwrap_or_default()
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: Server::default(),
            database: Database::default(),
            embedding: Embedding::default(),
        }
    }
}
