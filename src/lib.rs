//! Real time parsing and rendering of data coming from a TimeTagger.

pub mod configuration;
pub mod gui;
pub mod point_cloud_renderer;
pub mod rendering_helpers;

use std::path::PathBuf;
use std::{
    fs::{create_dir_all, read_to_string, write, File},
    num::{ParseFloatError, ParseIntError},
};

#[macro_use]
extern crate log;
#[macro_use]
extern crate lazy_static;
use anyhow::Result;
use directories::ProjectDirs;
use iced::Settings;
use kiss3d::{point_renderer::PointRenderer, renderer::Renderer};
use kiss3d::window::Window;
use nalgebra::Point3;
use pyo3::prelude::*;
use thiserror::Error;

use crate::configuration::{AppConfig, AppConfigBuilder};
use crate::gui::{ChannelNumber, EdgeDetected};
use crate::point_cloud_renderer::{PointDisplay, AppState, TimeTaggerIpcHandler};

const TT_DATA_STREAM: &str = "__tt_data_stream.dat";
const CALL_TIMETAGGER_SCRIPT_NAME: &str = "rpysight/call_timetagger.py";
pub const DEFAULT_CONFIG_FNAME: &str = "default.toml";
const TT_RUN_FUNCTION_NAME: &str = "run_tagger";
const TT_REPLAY_FUNCTION_NAME: &str = "replay_existing";
const GLOBAL_OFFSET: i64 = 0;

lazy_static! {
    /// GREEN, MAGENTA, CYAN, GRAY
    static ref DISPLAY_COLORS: [Point3<f32>; 4] = [Point3::<f32>::new(0.0, 1.0, 0.0), Point3::<f32>::new(1.0, 0.0, 1.0), Point3::<f32>::new(0.0, 1.0, 1.0), Point3::<f32>::new(1.0, 1.0, 1.0)];
}

/// Load an existing configuration file or generate a new one with default
/// values and load that instead.
///
/// Configuration files are stored in their proper locations using the
/// directories cargo package.
pub fn reload_cfg_or_use_default(config_name: Option<PathBuf>) -> AppConfig {
    let config_path = get_config_path(config_name);
    if config_path.exists() {
        read_to_string(config_path)
            .map(|res| toml::from_str(&res))
            .expect("Read to string failed")
            .expect("TOML parsing failed")
    } else {
        info!("Creating new configuration file in {:?}", config_path);
        create_dir_and_populate_with_default(config_path)
            .unwrap_or_else(|_| AppConfigBuilder::default().build())
    }
}
        

/// Start the renderer.
///
/// Does the needed setup to generate the window and app objects that are used
/// for rendering.
pub fn setup_renderer<T: Renderer + PointDisplay>(window: &mut Window, renderer: T, app_config: &AppConfig, data_stream_fh: String) -> AppState<T, File> {
    let frame_rate = app_config.frame_rate().round() as u64;
    window.set_framerate_limit(Some(frame_rate)); 
    let app = AppState::new(renderer, data_stream_fh, app_config.clone());
    app
}

/// Generates a PathBuf with the location of the default configuration path.
///
/// This function doesn't assert that it exists, it simply returns it.
pub(crate) fn get_config_path(config_name: Option<PathBuf>) -> PathBuf {
    let config_path = if let Some(proj_dirs) = ProjectDirs::from("lab", "PBLab", "RPySight") {
        proj_dirs.config_dir().join(config_name.unwrap_or(PathBuf::from(DEFAULT_CONFIG_FNAME)))
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
    settings.window.size = (800, 1200);
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
/// "call1" method that starts the TT.
pub fn load_timetagger_run_function(
    module_filename: PathBuf,
    replay_existing: bool,
) -> PyResult<PyObject> {
    let gil = Python::acquire_gil();
    let py = gil.python();
    let python_code = read_to_string(module_filename)?;
    let run_tt = PyModule::from_code(py, &python_code, "run_tt.py", "run_tt")?;
    let tt_starter;
    if replay_existing {
        tt_starter = run_tt.getattr(TT_REPLAY_FUNCTION_NAME)?;
    } else {
        tt_starter = run_tt.getattr(TT_RUN_FUNCTION_NAME)?;
    };
    // Generate an owned object to be returned by value
    Ok(tt_starter.to_object(py))
}

pub fn start_timetagger_with_python(app_config: &AppConfig) -> PyResult<()> {
    let module_filename = PathBuf::from(CALL_TIMETAGGER_SCRIPT_NAME);
    let tt_module = load_timetagger_run_function(module_filename, app_config.replay_existing)?;
    tt_module
        .call1(
            Python::acquire_gil().python(),
            (toml::to_string(app_config).expect("Unable to convert configuration to string"),),
        )
        .expect("Starting the TimeTagger failed, aborting!");
    Ok(())
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

/// Initializes things on the Python side and starts the acquisition.
///
/// This method is called once the user clicks the "Run Application" button or
/// from the CLI.
pub async fn start_acquisition(config_name: PathBuf, cfg: AppConfig) {
    let _ = save_cfg(Some(config_name), &cfg).ok(); // Errors are logged and quite irrelevant
    let mut window = Window::new("rPySight 0.1.0");
    let mut app = setup_renderer(&mut window, PointRenderer::new(), &cfg, TT_DATA_STREAM.to_string());
    debug!("Renderer set up correctly");
    std::thread::spawn(move || {
        start_timetagger_with_python(&cfg).expect("Failed to start TimeTagger, aborting")
    });
    app.acquire_stream_filehandle()
        .expect("Failed to acquire stream handle");
    window.render_loop(app);
}

/// Saves the current configuration to disk.
///
/// This function is called when the user starts the acquisition, which
/// means that it can assume that the config exists, since it's usually
/// created during start up.
///
/// The function overwrites the current settings with the new ones, as we
/// don't currently offer any profiles\configuration management system.
///
/// Errors during this function are called and then basically discarded,
/// since it's not "mission critical".
fn save_cfg(config_name: Option<PathBuf>, app_config: &AppConfig) -> anyhow::Result<()> {
    let config_path = get_config_path(config_name);
    if config_path.exists() {
        let serialized_cfg = toml::to_string(app_config).map_err(|e| {
            warn!(
                "Couldn't serialize user input struct before writing to disk: {}",
                e
            );
            e
        })?;
        write(&config_path, serialized_cfg).map_err(|e| {
            warn!("Couldn't serialize user input to disk: {}", e);
            e
        })?;
    } else {
        warn!("Configuration path doesn't exist before running the app");
    };
    debug!("Config saved successfully");
    Ok(())
}
