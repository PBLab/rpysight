//! Real time parsing and rendering of data coming from a TimeTagger.
//!
//! This crate is designed to render images or volumes of Two Photon
//! Microscopes that are imaged using the TimeTagger, a unique discriminator
//! and digitizer made by Swabian Instruments. Plugging in your typical Two
//! Photon hardware into a TT turns your microscope into a photon-counting
//! based device, which results in improved imaging conditions. However,
//! without rPySight it's difficult to see the full extent of improvement that
//! photon counting provides in real time, and offline-based solutions are
//! required.
//!
//! rPySight alleviates this requirement while also providing experimenters
//! with easier integration of voluemtric scanning via a TAG lens, a resonant
//! Z-axis scanner that is very fast but hard to integrate into a standard Two
//! Photon Microscope since it can't be controlled by an external signal.
//!
//! Taken together, rPySight facilitates rapid voluemtric scanning of live
//! tissue with realtime visualization of the data.

pub mod configuration;
pub mod event_stream;
pub mod gui;
pub mod point_cloud_renderer;
pub mod snakes;

use std::net::TcpStream;
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
use nalgebra::Point3;
use pyo3::prelude::*;
use thiserror::Error;

use crate::configuration::{AppConfig, AppConfigBuilder, InputChannel};
use crate::gui::{ChannelNumber, EdgeDetected};
use crate::point_cloud_renderer::{AppState, Channels, DisplayChannel};

/// The port we use to transfer data from the Python process controlling the TT
/// to the renderer.
const TT_DATA_STREAM: &str = "127.0.0.1:64444";
/// Filename with the code running the TT
const CALL_TIMETAGGER_SCRIPT_NAME: &str = "rpysight/call_timetagger.py";
/// Default configuration filename
pub const DEFAULT_CONFIG_FNAME: &str = "default.toml";
/// The function name that runs the TT with new data
const TT_RUN_FUNCTION_NAME: &str = "run_tagger";
/// The function name that runs the TT in replay mode
const TT_REPLAY_FUNCTION_NAME: &str = "replay_existing";
/// The gray level step that each photon adds to the current pixel. This is a
/// poor man's brightness normalization mechanism
const COLOR_INCREMENT: f32 = 2.0;

lazy_static! {
    /// Brightness starting level of each channel
    static ref GRAYSCALE_START: Point3<f32> = Point3::<f32>::new(0.05, 0.05, 0.05);
    /// GRAY, GREEN, MAGENTA, CYAN
    static ref DISPLAY_COLORS: [Point3<f32>; 4] = [Point3::<f32>::new(0.05, 0.05, 0.05), Point3::<f32>::new(0.0, 0.05, 0.0), Point3::<f32>::new(0.05, 0.0, 0.05), Point3::<f32>::new(0.0, 0.05, 0.05)];
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

/// Generates a PathBuf with the location of the default configuration path.
///
/// This function doesn't assert that it exists, it simply returns it.
pub(crate) fn get_config_path(config_name: Option<PathBuf>) -> PathBuf {
    let config_path = if let Some(proj_dirs) = ProjectDirs::from("lab", "PBLab", "RPySight") {
        proj_dirs
            .config_dir()
            .join(config_name.unwrap_or(PathBuf::from(DEFAULT_CONFIG_FNAME)))
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
    debug!("Starting timetagger");
    let module_filename = PathBuf::from(CALL_TIMETAGGER_SCRIPT_NAME);
    let tt_module = load_timetagger_run_function(module_filename, app_config.replay_existing)?;
    tt_module
        .call1(
            Python::acquire_gil().python(),
            (toml::to_string(app_config).expect("Unable to convert configuration to string"),),
        )
        .expect("Starting the TimeTagger failed, aborting!");
    debug!("Called Python to start the TT business");
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
fn channel_value_to_pair(ch: InputChannel) -> (ChannelNumber, EdgeDetected, f32) {
    let ch_no_edge = ch.channel.abs();
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
    let edge = match ch.channel.signum() {
        0 | 1 => EdgeDetected::Rising,
        -1 => EdgeDetected::Falling,
        _ => unreachable!(),
    };
    (chnum, edge, ch.threshold)
}

fn generate_windows(width: u32, height: u32, fr: u64) -> Channels<DisplayChannel> {
    let channel_names = [
        "Channel 1",
        "Channel 2",
        "Channel 3",
        "Channel 4",
        "Channel Merge",
    ];
    let mut channels = Vec::new();
    for name in channel_names.iter() {
        channels.push(DisplayChannel::new(*name, width, height, fr));
    }
    Channels::new(channels)
}

/// Initializes things on the Python side and starts the acquisition.
///
/// This method is called once the user clicks the "Run Application" button or
/// from the CLI.
pub async fn start_acquisition(config_name: PathBuf, cfg: AppConfig) {
    let _ = save_cfg(Some(config_name), &cfg).ok(); // errors are logged and quite irrelevant
    let fr = (&cfg).frame_rate().round() as u64;
    let channels = generate_windows(cfg.rows, cfg.columns, fr);
    let mut app = AppState::<DisplayChannel, TcpStream>::new(
        channels,
        TT_DATA_STREAM.to_string(),
        cfg.clone(),
    );
    debug!("Renderer set up correctly");
    let cloned_cfg = cfg.clone();
    std::thread::spawn(move || {
        start_timetagger_with_python(&cloned_cfg).expect("Failed to start TimeTagger, aborting")
    });
    app.start_inf_acq_loop(cfg).expect("Some error during acq");
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

/// Setup the logger. We're not using color here because terminals are either
/// slow in rendering them, or their simply not supported.
pub fn setup_logger(fname: Option<PathBuf>) {
    let log_fname;
    if let Some(f) = fname {
        log_fname = f
    } else {
        log_fname = PathBuf::from("target/test_rpysight.log");
    };

    fern::Dispatch::new()
        .format(move |out, message, record| {
            out.finish(format_args!(
                "{date} [{target}] [{level}] {message}",
                date = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.9f"),
                target = record.target(),
                level = record.level(),
                message = message,
            ));
        })
        .level(log::LevelFilter::Trace)
        .chain(File::create(log_fname).unwrap())
        .apply()
        .unwrap();
}
