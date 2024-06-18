use anyhow::Result;
use serde::{Deserialize, Serialize};

const APP_CONFIG_FILE: &str = "config.ron";

fn default_root_folder() -> String {
    "app".to_string()
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AppConfig {
    #[serde(default = "default_root_folder")]
    pub root_folder: String,
}

impl AppConfig {
    pub fn load() -> Result<Self> {
        if let Ok(config_str) = std::fs::read_to_string(APP_CONFIG_FILE) {
            let config: Self = ron::de::from_str(&config_str)?;
            Ok(config)
        } else {
            Ok(Self {
                root_folder: default_root_folder(),
            })
        }
    }

    pub fn save(&self) -> Result<()> {
        let config_str = ron::ser::to_string_pretty(&self, Default::default())?;
        std::fs::write(APP_CONFIG_FILE, config_str)?;
        Ok(())
    }
}
