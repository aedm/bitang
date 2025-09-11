use std::sync::OnceLock;

use anyhow::Result;
use serde::{Deserialize, Serialize};

const APP_CONFIG_FILE: &str = "config.ron";
static APP_CONFIG: OnceLock<AppConfig> = OnceLock::new();

fn default_root_folder() -> String {
    "demo".to_string()
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct AppConfig {
    #[serde(default = "default_root_folder")]
    pub root_folder: String,

    #[serde(default)]
    pub start_in_demo_mode: bool,
}

impl AppConfig {
    pub fn get() -> &'static AppConfig {
        APP_CONFIG.get_or_init(|| Self::load())
    }

    fn load() -> Self {
        if let Ok(config_str) = std::fs::read_to_string(APP_CONFIG_FILE) {
            match ron::de::from_str(&config_str) {
                Ok(app_config) => app_config,
                Err(err) => panic!("Invalid config file context: {err:?}")
            }
        } else {
            Self {
                root_folder: default_root_folder(),
                ..Default::default()
            }
        }
    }
}
