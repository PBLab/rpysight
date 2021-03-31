//! Real time parsing and rendering of data coming from a TimeTagger

pub mod gui;
mod interval_tree;
mod photon;
pub mod point_cloud_renderer;
mod rendering_helpers;

use std::fs::{read_to_string, File};
use std::path::PathBuf;

use kiss3d::window::Window;
use pyo3::prelude::*;
use thiserror::Error;

use crate::gui::{ConfigGui, ChannelNumber, EdgeDetected};
use crate::point_cloud_renderer::{setup_renderer, AppState};
use crate::rendering_helpers::{AppConfig, AppConfigBuilder, Period, Picosecond};

const TT_DATA_STREAM: &'static str = "__tt_data_stream.dat";
const CALL_TIMETAGGER_SCRIPT_NAME: &'static str = "rpysight/call_timetagger.py";

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
    // Generate an owned object to be returned by value
    Ok(tt_starter.to_object(py))
}

pub(crate) fn setup_rpysight(config_gui: &ConfigGui) -> (Window, AppState<File>) {
    // Set up the Python side
    let filename = PathBuf::from(CALL_TIMETAGGER_SCRIPT_NAME);
    let timetagger_module: PyObject =
        load_timetagger_module(filename).expect("Python file and process could not be hooked into");
    let gil = Python::acquire_gil();
    // Set up the renderer side
    let (window, app) = setup_renderer(gil, timetagger_module, TT_DATA_STREAM.into(), config_gui);
    (window, app)
}

#[derive(Debug, Error, PartialEq)]
pub(crate) enum UserInputError {
    #[error("Wrong input given for rows field (got `{0}`)")]
    InvalidRows(String),
    #[error("Wrong input given for columns field (got `{0}`)")]
    InvalidColumns(String),
    #[error("Unknown user input error")]
    Unknown,
}

impl From<std::num::ParseIntError> for UserInputError {
    fn from(e: std::num::ParseIntError) -> UserInputError {
        UserInputError::Unknown
    }
}

impl From<std::num::ParseFloatError> for UserInputError {
    fn from(e: std::num::ParseFloatError) -> UserInputError {
        UserInputError::Unknown
    }
}

pub(crate) fn parse_user_input_into_config(
    user_input: &ConfigGui,
) -> Result<AppConfig, UserInputError> {
    Ok(AppConfigBuilder::default()
        .with_rows(user_input.get_num_rows().parse::<u32>()?)
        .with_columns(user_input.get_num_columns().parse::<u32>()?)
        .with_planes(user_input.get_num_planes().parse::<u32>()?)
        .with_bidir(user_input.get_bidirectionality().into())
        .with_tag_period(Period::from_freq(user_input.get_taglens_period().parse::<f64>()?))
        .with_scan_period(Period::from_freq(user_input.get_scan_period().parse::<f64>()?))
        .with_fill_fraction(user_input.get_fill_fraction().parse::<f32>()?)
        .with_frame_dead_time(user_input.get_frame_dead_time().parse::<Picosecond>()? * 1_000_000_000)
        .with_pmt1_ch(convert_user_channel_input_to_num(user_input.get_pmt1_channel()))
        .with_pmt2_ch(convert_user_channel_input_to_num(user_input.get_pmt2_channel()))
        .with_pmt3_ch(convert_user_channel_input_to_num(user_input.get_pmt3_channel()))
        .with_pmt4_ch(convert_user_channel_input_to_num(user_input.get_pmt4_channel()))
        .with_laser_ch(convert_user_channel_input_to_num(user_input.get_laser_channel()))
        .with_frame_ch(convert_user_channel_input_to_num(user_input.get_frame_channel()))
        .with_line_ch(convert_user_channel_input_to_num(user_input.get_line_channel()))
        .with_taglens_ch(convert_user_channel_input_to_num(user_input.get_tag_channel()))
        .build())
}


fn convert_user_channel_input_to_num(channel: (ChannelNumber, EdgeDetected)) -> i32 {
    let edge: i32 = match channel.1 {
        EdgeDetected::Rising => 1,
        EdgeDetected::Falling => -1,
    };
    edge * match channel.0 {
        ChannelNumber::Channel1 => 1,
        ChannelNumber::Channel2 => 2,
        ChannelNumber::Channel3 => 3,
        ChannelNumber::Channel4 => 4,
        ChannelNumber::Channel5 => 5,
        ChannelNumber::Channel6 => 6,
        ChannelNumber::Channel7 => 7,
        ChannelNumber::Channel8 => 8,
        ChannelNumber::Channel9 => 9,
        ChannelNumber::Channel10 => 10,
        ChannelNumber::Channel11 => 11,
        ChannelNumber::Channel12 => 12,
        ChannelNumber::Channel13 => 13,
        ChannelNumber::Channel14 => 14,
        ChannelNumber::Channel15 => 15,
        ChannelNumber::Channel16 => 16,
        ChannelNumber::Channel17 => 17,
        ChannelNumber::Channel18 => 18,
        ChannelNumber::Empty => 0,
    }
}
