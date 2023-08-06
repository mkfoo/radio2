pub mod config;
pub mod error;
pub mod hls;
pub mod meta;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
