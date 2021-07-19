use std::env;
use std::fs::read_to_string;
use std::path::PathBuf;
use std::ffi::OsStr;

#[macro_use]
extern crate log;

use futures::executor::block_on;
use anyhow::Result;
use thiserror::Error;

use librpysight::configuration::AppConfig;
use librpysight::{start_acquisition, setup_logger};

#[derive(Debug, Error)]
pub enum ConfigParsingError {
    #[error("File not found (received {0})")]
    FileNotFound(PathBuf),
    #[error("Expected TOML extension (found {0})")]
    WrongExtension(String),
    #[error("Missing configuration file, please provide one as an argument")]
    MissingConfig,
}

/// Asserts that the argument list to our software was given according to the
/// specs
fn validate_and_parse_args(args: &[String]) -> Result<PathBuf, ConfigParsingError> {
    if !args.len() != 1 {
        return Err(ConfigParsingError::MissingConfig)
    }
    let path = PathBuf::from(&args[0]);
    if !path.exists() {
        return Err(ConfigParsingError::FileNotFound(path))
    }
    if path.extension() != Some(OsStr::new("toml")) {
        return Err(ConfigParsingError::WrongExtension("Wrong file given (expected TOML)".to_string()))
    };
    Ok(path)
}

/// Runs rPySight from the CLI
fn main() -> Result<()> {
    setup_logger(Some(PathBuf::from("target/rpysight.log")));
    info!("Logger initialized successfully, starting rPySight from the CLI");
    let args: Vec<String> = env::args().collect();
    let config_path = validate_and_parse_args(&args[1..])?;
    let config: AppConfig = toml::from_str(&read_to_string(&config_path)?)?;
    block_on(start_acquisition(config_path, config));
    Ok(())
}
