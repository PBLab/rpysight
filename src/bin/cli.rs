// Remember to $Env:PYTHONHOME = "C:\Users\PBLab\.conda\envs\timetagger\"
// because powershell is too dumb to remember.
use std::env;
use std::ffi::OsStr;
use std::fs::read_to_string;
use std::path::PathBuf;

#[macro_use]
extern crate log;

use anyhow::Result;
use futures::executor::block_on;
use thiserror::Error;

use librpysight::configuration::AppConfig;
use librpysight::{setup_logger, start_acquisition};

#[derive(Debug, Error)]
pub enum ConfigParsingError {
    #[error("File not found (received {0})")]
    FileNotFound(PathBuf),
    #[error("Expected TOML extension (found {0})")]
    WrongExtension(String),
    #[error("Missing configuration file, please provide one as an argument")]
    MissingConfig,
}

pub struct ValidatedArgs {
    pub path: PathBuf,
}

struct ArgsWithCorrectExtension {
    pub path: PathBuf,
}

impl ArgsWithCorrectExtension {
    pub fn parse(self) -> Result<ValidatedArgs, ConfigParsingError> {
        if self.path.extension() != Some(OsStr::new("toml")) {
            return Err(ConfigParsingError::WrongExtension(
                "Wrong file given (expected TOML)".to_string(),
            ));
        } else {
            Ok(ValidatedArgs { path: self.path })
        }
    }
}

struct ArgsThatExistOnDisk {
    pub path: PathBuf,
}

impl ArgsThatExistOnDisk {
    pub fn parse(self) -> Result<ArgsWithCorrectExtension, ConfigParsingError> {
        if !self.path.exists() {
            return Err(ConfigParsingError::FileNotFound(self.path));
        } else {
            Ok(ArgsWithCorrectExtension { path: self.path })
        }
    }
}

struct CorrectNumberOfArgs<'a> {
    pub args: &'a [String],
}

impl<'a> CorrectNumberOfArgs<'a> {
    pub fn parse(self) -> Result<ArgsThatExistOnDisk, ConfigParsingError> {
        if self.args.len() != 1 {
            return Err(ConfigParsingError::MissingConfig);
        } else {
            Ok(ArgsThatExistOnDisk {
                path: PathBuf::from(&self.args[0]),
            })
        }
    }
}
/// Asserts that the argument list to our software was given according to the
/// specs
fn validate_and_parse_args(args: &[String]) -> Result<PathBuf, ConfigParsingError> {
    let validated = CorrectNumberOfArgs { args }
        .parse()
        .and_then(|exist_on_disk| {
            exist_on_disk
                .parse()
                .and_then(|correct_extension| correct_extension.parse())
        })?;
    Ok(validated.path)
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
