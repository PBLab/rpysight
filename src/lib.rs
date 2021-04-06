//! Real time parsing and rendering of data coming from a TimeTagger.

pub mod gui;
pub mod point_cloud_renderer;
pub mod rendering_helpers;

use std::{fs::{File, create_dir_all, read_to_string, write}, num::{ParseFloatError, ParseIntError}};
use std::path::PathBuf;

#[macro_use]
extern crate log;
use anyhow::Result;
use directories::ProjectDirs;
use iced::Settings;
use kiss3d::window::Window;
use pyo3::prelude::*;
use thiserror::Error;
use toml;

use crate::gui::{ChannelNumber, EdgeDetected};
use crate::point_cloud_renderer::{setup_renderer, AppState};
use crate::rendering_helpers::{AppConfig, AppConfigBuilder};

const TT_DATA_STREAM: &'static str = "__tt_data_stream.dat";
const CALL_TIMETAGGER_SCRIPT_NAME: &'static str = "rpysight/call_timetagger.py";
const DEFAULT_CONFIG_FNAME: &'static str = "default.toml";

/// Load an existing configuration file or generate a new one with default
/// values and load that instead.
///
/// Configuration files are stored in their proper locations using the
/// directories cargo package.
pub fn reload_cfg_or_use_default() -> AppConfig {
    let config_path = get_config_path();
    if config_path.exists() {
        read_to_string(config_path)
            .and_then(|res| Ok(toml::from_str(&res)))
            .expect("Read to string failed")
            .expect("TOML parsing failed")
    } else {
        info!("Creating new configuration file in {:?}", config_path);
        create_dir_and_populate_with_default(config_path)
            .unwrap_or(AppConfigBuilder::default().build())
    }
}

/// Generates a PathBuf with the location of the default configuration path.
///
/// This function doesn't assert that it exists, it simply returns it.
pub(crate) fn get_config_path() -> PathBuf {
    let config_path = if let Some(proj_dirs) = ProjectDirs::from("lab", "PBLab", "RPySight") {
        proj_dirs.config_dir().join(DEFAULT_CONFIG_FNAME)
    } else {
        // Unreachable since config_dir() doesn't fail or returns None
        unreachable!()
    };
    info!("Configuration path: {:?}", config_path);
    config_path
}


/// Populates a Settings instance with the configuration of RPySight.
///
/// If any additional changes to the default settings should be made, then
/// they should be done inside this function.
pub fn load_app_settings(cfg: AppConfig) -> Settings<AppConfig> {
    let mut settings = Settings::with_flags(cfg);
    settings.window.size = (800, 1100);
    settings
}

/// Writes a default configuration file to disk and returns it.
///
/// This functions is called in the case that RPySight is run for the first
/// time in a workstation and the configuration folder and files don't yet
///  exist. It writes a new default file to disk and returns it. If failed
///  during this process it will log these errors to disk and returns an Err
///  variant, which will be handled upstream.
fn create_dir_and_populate_with_default(path: PathBuf) -> Result<AppConfig> {
    let default_cfg = AppConfigBuilder::default().build();
    let seralized_cfg = toml::to_string(&default_cfg).map_err(|e| {
        warn!("Error serializing configuration to TOML: {:?}", e);
        e
    })?;
    if let Some(prefix) = path.parent() {
        let _ = create_dir_all(prefix)?;
    }
    let _ = write(&path, seralized_cfg).map_err(|e| {
        warn!("Error writing serialized configuration to disk: {:?}", e);
        e
    })?;
    Ok(default_cfg)
}

/// Loads the Python file with the TimeTagger start up script.
///
/// The given filename should point to a Python file that can run the
/// TimeTagger with a single method call. The returned object will have a
/// "call0" method that starts the TT.
pub fn load_timetagger_module(fname: PathBuf) -> PyResult<PyObject> {
    let gil = Python::acquire_gil();
    let py = gil.python();
    let python_code = read_to_string(fname)?;
    let run_tt = PyModule::from_code(py, &python_code, "run_tt.py", "run_tt")?;
    let tt_starter = run_tt.getattr("run_tagger")?;
    info!("Python module loaded successfully");
    // Generate an owned object to be returned by value
    Ok(tt_starter.to_object(py))
}

/// A few necessary setups steps before starting the acquisition.
pub(crate) fn setup_rpysight(app_config: &AppConfig) -> (Window, AppState<File>) {
    // Set up the Python side
    let filename = PathBuf::from(CALL_TIMETAGGER_SCRIPT_NAME);
    let timetagger_module: PyObject =
        load_timetagger_module(filename).expect("Python file and process could not be hooked into");
    let gil = Python::acquire_gil();
    // Set up the renderer side
    let (window, app) = setup_renderer(gil, timetagger_module, TT_DATA_STREAM.into(), app_config);
    info!("Renderer setup completed");
    (window, app)
}

/// A custom error returned when the user supplies incorrect values.
#[derive(Debug, Error, PartialEq)]
pub enum UserInputError {
    #[error("Wrong input given for rows field (got `{0}`)")]
    InvalidRows(ParseIntError),
    #[error("Wrong input given for columns field (got `{0}`)")]
    InvalidColumns(ParseIntError),
    #[error("Wrong input given for planes field (got `{0}`)")]
    InvalidPlanes(ParseIntError),
    #[error("Wrong TAG Lens period value (got `{0}`)")]
    InvalidTagLensPeriod(ParseFloatError),
    #[error("Wrong scan period value (got `{0}`)")]
    InvalidScanPeriod(ParseFloatError),
    #[error("Wrong frame dead time value (got `{0}`)")]
    InvalidFrameDeadTime(ParseFloatError),
    #[error("Unknown user input error")]
    Unknown,
}

impl From<std::num::ParseIntError> for UserInputError {
    fn from(_e: std::num::ParseIntError) -> UserInputError {
        UserInputError::Unknown
    }
}

impl From<std::num::ParseFloatError> for UserInputError {
    fn from(_e: std::num::ParseFloatError) -> UserInputError {
        UserInputError::Unknown
    }
}

/// Converts a TT representation of a channel into its corresponding
/// ChannelNumber and EdgeDetected pairs.
///
/// The TimeTagger uses the sign of the number to signal the edge, and the
/// value obviously corresponds to the channel number.
fn channel_value_to_pair(ch: i32) -> (ChannelNumber, EdgeDetected) {
    let ch_no_edge = ch.abs();
    let chnum = match ch_no_edge {
        0 => ChannelNumber::Disconnected,
        1 => ChannelNumber::Channel1,
        2 => ChannelNumber::Channel2,
        3 => ChannelNumber::Channel3,
        4 => ChannelNumber::Channel4,
        5 => ChannelNumber::Channel5,
        6 => ChannelNumber::Channel6,
        7 => ChannelNumber::Channel7,
        8 => ChannelNumber::Channel8,
        9 => ChannelNumber::Channel9,
        10 => ChannelNumber::Channel10,
        11 => ChannelNumber::Channel11,
        12 => ChannelNumber::Channel12,
        13 => ChannelNumber::Channel13,
        14 => ChannelNumber::Channel14,
        15 => ChannelNumber::Channel15,
        16 => ChannelNumber::Channel16,
        17 => ChannelNumber::Channel17,
        18 => ChannelNumber::Channel18,
        _ => panic!("Invalid channel detected!"),
    };
    let edge = match ch.signum() {
        0 | 1 => EdgeDetected::Rising,
        -1 => EdgeDetected::Falling,
        _ => unreachable!(),
    };
    (chnum, edge)
}
