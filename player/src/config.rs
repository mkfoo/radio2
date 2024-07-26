use serde::Deserialize;
use std::collections::hash_map::HashMap;
use std::env;
use std::path::Path;

#[derive(Deserialize, Debug, Clone)]
pub struct Channel {
    pub manifest_url: String,
    pub name: String,
    service_id: String,
}

fn read_config(path: &Path) -> super::Result<String> {
    let s = std::fs::read_to_string(path)?;
    println!("read config from {}", path.display());
    Ok(s)
}

fn read_from_cwd() -> super::Result<String> {
    let path = env::current_dir().map(|p| p.join("radio.json"))?;
    read_config(path.as_path())
}

fn read_from_home_dir() -> super::Result<String> {
    let path = env::var("HOME").map(|s| Path::new(&s).join(".config/radio.json"))?;
    read_config(path.as_path())
}

fn read_from_etc() -> super::Result<String> {
    let path = Path::new("/etc/radio.json");
    read_config(path)
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
        let text = read_from_cwd()
            .or_else(|_| read_from_home_dir())
            .or_else(|_| read_from_etc())?;
        let conf: Self = serde_json::from_str(&text)?;
        Ok(conf)
    }
}
