use std::env;
use std::fs::{read_to_string, File};
use std::path::PathBuf;
use std::ffi::OsStr;

#[macro_use]
extern crate log;
extern crate simplelog;

use simplelog::*;
use futures::executor::block_on;
use anyhow::Result;
use thiserror::Error;

use librpysight::configuration::AppConfig;
use librpysight::start_acquisition;

#[derive(Debug, Error)]
pub enum ConfigParsingError {
    #[error("File not found (received {0})")]
    FileNotFound(PathBuf),
    #[error("Expected TOML extension (found {0})")]
    WrongExtension(String),
}

/// Asserts that the argument list to our software was given according to the
/// specs
fn validate_and_parse_args(args: &[String]) -> Result<PathBuf, ConfigParsingError> {
    assert_eq!(args.len(), 1);
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
    let _ = WriteLogger::init(
        LevelFilter::Trace,
        ConfigBuilder::default().set_time_to_local(true).build(),
        File::create("target/rpysight.log")?,
    );
    info!("Logger initialized successfully, starting rPySight from the CLI");
    let args: Vec<String> = env::args().collect();
    let config_path = validate_and_parse_args(&args[1..])?;
    let config: AppConfig = toml::from_str(&read_to_string(&config_path)?)?;
    block_on(start_acquisition(config_path, config));
    Ok(())
}
