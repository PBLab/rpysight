use futures::executor::block_on;
use std::env;
use std::fs::{read_to_string, File};
use std::io::{Error, ErrorKind, Result};
use std::path::PathBuf;

#[macro_use]
extern crate log;
extern crate simplelog;

use simplelog::*;

use librpysight::configuration::AppConfig;
use librpysight::start_acquisition;

/// Asserts that the argument list to our software was given according to the
/// specs
fn validate_and_parse_args(args: &[String]) -> Result<PathBuf> {
    assert_eq!(args.len(), 1);
    let path = PathBuf::from(&args[0]);
    if !path.exists() {
        return Err(Error::new(ErrorKind::NotFound, "Given file not found"));
    }
    assert!(path.exists(), "File doesn't exist");
    assert_eq!(
        path.extension().ok_or(ErrorKind::InvalidInput)?,
        "toml",
        "Wrong file given (expected TOML)"
    );
    Ok(path)
}

/// Runs rPySight from the CLI
fn main() -> Result<()> {
    let _ = WriteLogger::init(
        LevelFilter::Info,
        ConfigBuilder::default().set_time_to_local(true).build(),
        File::create("target/rpysight.log")?,
    );
    info!("Logger initialized successfully, starting rPySight from the CLI");
    println!("rPySight 0.1.0");
    let args: Vec<String> = env::args().collect();
    let config_path = validate_and_parse_args(&args[1..])?;
    let config: AppConfig = toml::from_str(&read_to_string(config_path)?)?;
    block_on(start_acquisition(config));
    Ok(())
}
