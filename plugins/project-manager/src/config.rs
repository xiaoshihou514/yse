use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct ModelConfig {
    #[serde(default = "default_base_url")]
    pub base_url: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_model")]
    pub model: String,
}

fn default_base_url() -> String {
    "http://localhost:11434".into()
}

fn default_model() -> String {
    "qwen2.5:7b".into()
}

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub models: Vec<ModelConfig>,
}

impl Config {
    pub fn load() -> Self {
        let path = match dirs::config_dir() {
            Some(d) => d.join("yse").join("pm.toml"),
            None => return Self { models: vec![] },
        };

        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => return Self { models: vec![] },
        };

        toml::from_str(&content).unwrap_or_else(|e| {
            eprintln!("Failed to parse {}: {e}", path.display());
            Self { models: vec![] }
        })
    }

    pub fn fallback_chain(&self) -> Vec<ModelConfig> {
        if self.models.is_empty() {
            vec![ModelConfig {
                base_url: default_base_url(),
                api_key: String::new(),
                model: default_model(),
            }]
        } else {
            self.models.clone()
        }
    }
}
