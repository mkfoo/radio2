use serde::Deserialize;
use std::collections::hash_map::HashMap;

const DEFAULT_CONFIG_PATH: &str = "/etc/radio.json";

#[derive(Deserialize, Debug, Clone)]
pub struct Channel {
    pub manifest_url: String,
    pub name: String,
    service_id: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Config {
    channels: Vec<Channel>,
    pub lcd_path: String,
    meta_params: HashMap<String, String>,
    pub meta_url1: String,
    pub meta_url2: String,
    pub queue_length: usize,
    pub sock_path: String,
    pub target_bandwidth: u64,
    pub user_agent: String,
}

impl Config {
    pub fn get_channel(&self, idx: usize) -> &Channel {
        assert!(idx != 0);
        &self.channels[idx - 1]
    }

    pub fn get_meta_params(&self, idx: usize) -> HashMap<String, String> {
        assert!(idx != 0);
        let mut params = self.meta_params.clone();
        params.insert(
            "serviceId".to_string(),
            self.channels[idx - 1].service_id.clone(),
        );
        params
    }

    pub fn load_default() -> super::Result<Self> {
        let text = std::fs::read_to_string(DEFAULT_CONFIG_PATH)?;
        let c: Self = serde_json::from_str(&text)?;
        Ok(c)
    }
}
