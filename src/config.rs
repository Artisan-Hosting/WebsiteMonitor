use std::fmt;

use artisan_middleware::{
    config::AppConfig,
    log,
    logger::LogLevel
};
use colored::Colorize;
use config::{Config, ConfigError, File};
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct AppSpecificConfig {
    pub interval_seconds: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct WebsiteConfig {
    pub urls: Vec<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct Settings {
    pub app: AppSpecificConfig,
    pub websites: WebsiteConfig,
}

pub fn load_settings() -> Result<Settings, ConfigError> {
    let mut settings = Config::builder();
    settings = settings.add_source(File::with_name("Config").required(false));
    let settings = settings.build()?;
    let app_settings: Settings = settings.get("settings")?;
    Ok(app_settings)
}

pub fn get_config() -> AppConfig {
    let mut config: AppConfig = match AppConfig::new() {
        Ok(loaded_data) => loaded_data,
        Err(e) => {
            log!(LogLevel::Error, "Couldn't load config: {}", e.to_string());
            std::process::exit(0)
        }
    };
    config.app_name = env!("CARGO_PKG_NAME").to_string();
    config.version = env!("CARGO_PKG_VERSION").to_string();
    config.database = None;
    config.aggregator = None;
    config
}

impl fmt::Display for AppSpecificConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}\n  {}",
            "AppSpecificConfig:".bold().blue(),
            format!("Interval Seconds: {}", self.interval_seconds).green()
        )
    }
}

// Implement Display for WebsiteConfig
impl fmt::Display for WebsiteConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}\n  {}", "WebsiteConfig:".bold().blue(), "URLs: \n".yellow())?;
        for (index, url) in self.urls.iter().enumerate() {
            writeln!(f, "    {}. {}", (index + 1).to_string().cyan(), url.magenta())?;
        }
        Ok(())
    }
}

// Implement Display for Settings
impl fmt::Display for Settings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}\n\n{}",
            self.app, self.websites
        )
    }
}