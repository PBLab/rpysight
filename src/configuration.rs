//! All things related to user-facing configurations.

use std::num::ParseFloatError;
use std::ops::{Deref, Index};

use nalgebra::Point3;
use serde::{Deserialize, Serialize};

use crate::gui::{ChannelNumber, EdgeDetected, MainAppGui};
use crate::snakes::Picosecond;
use crate::UserInputError;

/// Picosecond and Hz aware period
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct Period {
    pub period: Picosecond,
}

impl Period {
    /// Convert a Hz-based frequency into units of picoseconds
    pub fn from_freq<T: Into<f64>>(hz: T) -> Period {
        let hz = hz.into();
        Period {
            period: ((1.0 / hz) * 1e12).round() as Picosecond,
        }
    }

    pub(crate) fn to_hz(&self) -> f32 {
        (1.0f64 / (self.period as f64 / 1_000_000_000_000.0f64)) as f32
    }
}

impl Deref for Period {
    type Target = Picosecond;

    fn deref(&self) -> &Picosecond {
        &self.period
    }
}

/// Determines whether the scan was bidirectional or unidirectional
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum Bidirectionality {
    Bidir,
    Unidir,
}

impl From<bool> for Bidirectionality {
    fn from(bidir: bool) -> Bidirectionality {
        match bidir {
            true => Bidirectionality::Bidir,
            false => Bidirectionality::Unidir,
        }
    }
}

impl From<Bidirectionality> for bool {
    fn from(bidir: Bidirectionality) -> bool {
        match bidir {
            Bidirectionality::Bidir => true,
            Bidirectionality::Unidir => false,
        }
    }
}

/// Enumerates all possible data streams that can be handled by rPySight, like
/// PMT data, line sync events and so on.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Copy)]
pub enum DataType {
    Pmt1,
    Pmt2,
    Pmt3,
    Pmt4,
    Frame,
    Line,
    TagLens,
    Laser,
    /// A connected output which is unneeded in this experiment
    Unwanted,
    Invalid,
}

/// Physical number of the input SMA ports on the time tagger.
///
/// It's an i32 due to it having to interact with the channel values that
/// return from the time tagger stream, which are also i32.
const MAX_TIMETAGGER_INPUTS: i32 = 18;

/// A physical input port on the TimeTagger, having both a channel value
/// (positive if the threshold is positive, else negative) and a threshold
/// value that is set as the trigger of it.
#[derive(Debug, Copy, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputChannel {
    pub channel: i32,
    pub threshold: f32,
}

impl InputChannel {
    pub fn new(channel: i32, threshold: f32) -> Self {
        InputChannel { channel, threshold }
    }
}

/// A data structure which maps the input channel to the data type it relays.
///
/// The underlying storage is an array, and due to the way the Index trait is
/// implemented here we can index into an Inputs instance with a positive or
/// negative value without any conversions.
#[derive(Clone, Debug)]
pub struct Inputs([DataType; 2 * (MAX_TIMETAGGER_INPUTS as usize) + 1]);

impl Inputs {
    /// Generates a new Inputs instance. Panics if the input channels aren't
    /// unique or if a channel was accidently assigned to a non-existent input.
    pub fn from_config(config: &AppConfig) -> Inputs {
        let mut data = [DataType::Invalid; 2 * (MAX_TIMETAGGER_INPUTS as usize) + 1];
        let mut set = std::collections::HashSet::<i32>::new();
        let mut used_channels = 0;
        let mut needed_channels = vec![
            config.pmt1_ch,
            config.pmt2_ch,
            config.pmt3_ch,
            config.pmt4_ch,
            config.frame_ch,
            config.line_ch,
            config.taglens_ch,
            config.laser_ch,
        ];
        let mut datatypes = vec![
            DataType::Pmt1,
            DataType::Pmt2,
            DataType::Pmt3,
            DataType::Pmt4,
            DataType::Frame,
            DataType::Line,
            DataType::TagLens,
            DataType::Laser,
        ];

        let num_unwanted_channels = config.ignored_channels.len();
        let mut ignored_channels = vec![DataType::Unwanted; num_unwanted_channels];
        needed_channels.append(&mut config.ignored_channels.clone());
        datatypes.append(&mut ignored_channels);
        assert!(needed_channels.len() == datatypes.len());
        // Loop over a pair of input and the corresponding data type, but only
        // register the inputs which are actually used, i.e. different than 0.
        for (ch, dt) in needed_channels.into_iter().zip(datatypes).into_iter() {
            if ch.channel != 0 {
                set.insert(ch.channel);
                data[(MAX_TIMETAGGER_INPUTS + ch.channel) as usize] = dt;
                used_channels += 1;
            }
        }
        assert_eq!(
            set.len(),
            used_channels,
            "One of the channels was a duplicate"
        );
        let inps = Inputs(data);
        debug!("The inputs struct was constructed successfully: {:?}", inps);
        inps
    }

    pub fn get(&self, channel: i32) -> &DataType {
        let actual_idx = (MAX_TIMETAGGER_INPUTS + channel) as usize;
        if actual_idx >= self.0.len() {
            error!("orig channel: {}", channel);
            &DataType::Invalid
        } else {
            &self.0[actual_idx]
        }
    }
}

impl Index<i32> for Inputs {
    type Output = DataType;

    fn index(&self, channel: i32) -> &Self::Output {
        &self.0[(MAX_TIMETAGGER_INPUTS + channel) as usize]
    }
}

/// Configuration for the rendering application.
///
/// This struct contains all needed information for rPySight to render the
/// photon stream correctly. It's generated by a user entering parameters into
/// the GUI.
///
/// It can be serialized so that it can be saved on disk as a configuration
/// file, and it can also be sent from Rust to Python so that the TimeTagger
/// will be aware of the different channels in use.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppConfig {
    pub(crate) filename: String,
    pub(crate) ignored_channels: Vec<InputChannel>,
    pub(crate) rows: u32,
    pub(crate) columns: u32,
    pub(crate) planes: u32,
    pub(crate) fill_fraction: f32, // (0..100)
    pub(crate) frame_dead_time: Picosecond,
    pub(crate) replay_existing: bool,
    pub(crate) rolling_avg: u16,
    pub(crate) line_shift: Picosecond,
    pub(crate) bidir: Bidirectionality,
    pub(crate) point_color: Point3<f32>,
    pub(crate) scan_period: Period,
    pub(crate) tag_period: Period,
    pub(crate) pmt1_ch: InputChannel,
    pub(crate) pmt2_ch: InputChannel,
    pub(crate) pmt3_ch: InputChannel,
    pub(crate) pmt4_ch: InputChannel,
    pub(crate) laser_ch: InputChannel,
    pub(crate) frame_ch: InputChannel,
    pub(crate) line_ch: InputChannel,
    pub(crate) taglens_ch: InputChannel,
}

impl AppConfig {
    /// Parse the supplied user parameters, returning errors if illegal.
    ///
    /// Each field is parsed using either simple string to number parsing or more
    /// elaborate special functions for some designated special types.
    pub fn from_user_input(user_input: &MainAppGui) -> anyhow::Result<AppConfig, UserInputError> {
        Ok(AppConfigBuilder::default()
            .with_filename(user_input.get_filename().to_string())
            .with_rows(
                user_input
                    .get_num_rows()
                    .parse::<u32>()
                    .map_err(UserInputError::InvalidRows)?,
            )
            .with_columns(
                user_input
                    .get_num_columns()
                    .parse::<u32>()
                    .map_err(UserInputError::InvalidColumns)?,
            )
            .with_planes(
                user_input
                    .get_num_planes()
                    .parse::<u32>()
                    .map_err(UserInputError::InvalidPlanes)?,
            )
            .with_bidir(user_input.get_bidirectionality())
            .with_tag_period(Period::from_freq(
                user_input
                    .get_taglens_period()
                    .parse::<f64>()
                    .map_err(UserInputError::InvalidTagLensPeriod)?,
            ))
            .with_scan_period(Period::from_freq(
                user_input
                    .get_scan_period()
                    .parse::<f64>()
                    .map_err(UserInputError::InvalidScanPeriod)?,
            ))
            .with_fill_fraction(user_input.get_fill_fraction().parse::<f32>()?)
            .with_frame_dead_time(
                string_ms_to_ps(user_input.get_frame_dead_time())
                    .map_err(UserInputError::InvalidFrameDeadTime)?,
            )
            .with_pmt1_ch(convert_user_channel_input_to_num(
                user_input.get_pmt1_channel(),
            ))
            .with_pmt2_ch(convert_user_channel_input_to_num(
                user_input.get_pmt2_channel(),
            ))
            .with_pmt3_ch(convert_user_channel_input_to_num(
                user_input.get_pmt3_channel(),
            ))
            .with_pmt4_ch(convert_user_channel_input_to_num(
                user_input.get_pmt4_channel(),
            ))
            .with_laser_ch(convert_user_channel_input_to_num(
                user_input.get_laser_channel(),
            ))
            .with_frame_ch(convert_user_channel_input_to_num(
                user_input.get_frame_channel(),
            ))
            .with_line_ch(convert_user_channel_input_to_num(
                user_input.get_line_channel(),
            ))
            .with_taglens_ch(convert_user_channel_input_to_num(
                user_input.get_tag_channel(),
            ))
            .with_replay_existing(user_input.get_replay_existing())
            .with_rolling_avg(user_input.get_rolling_avg())
            .with_ignored_channels(convert_ignored_to_vec(user_input.get_ignored_channels()))
            .with_line_shift(user_input.get_line_shift().parse::<Picosecond>().unwrap())
            .build())
    }

    /// The time in ps it takes for a frame to complete. Not including the dead
    /// time between frames.
    pub fn calc_frame_duration(&self) -> Picosecond {
        match self.bidir {
            Bidirectionality::Bidir => (*self.scan_period / 2) * (self.rows as Picosecond),
            Bidirectionality::Unidir => *self.scan_period * (self.rows as Picosecond),
        }
    }

    /// Return the frame rate in Hz
    pub fn frame_rate(&self) -> f32 {
        Period {
            period: self.calc_frame_duration(),
        }
        .to_hz()
    }

    pub fn get_num_pixels(&self) -> usize {
        let planes = self.planes.max(1);
        let rows = self.rows.max(1);
        let columns = self.columns.max(1);
        (planes * columns * rows) as usize
    }
}

/// Converts a comma-separated list of numbers into channels
fn convert_ignored_to_vec(ignored_str: &str) -> Vec<InputChannel> {
    if ignored_str.len() == 0 {
        return vec![];
    };
    ignored_str
        .trim_end_matches(",")
        .replace(" ", "")
        .split(",")
        .map(|ch| ch.parse::<i32>().unwrap())
        .map(|ch| InputChannel::new(ch, 0.0))
        .collect()
}

/// Converts a miliseconds number (a string) into its equivalent in ps.
fn string_ms_to_ps(ms_as_string: &str) -> anyhow::Result<Picosecond, ParseFloatError> {
    let ms = ms_as_string.parse::<f64>()?;
    Ok((ms * 1_000_000_000f64) as Picosecond)
}

/// Converts a chosen user channel to its TT representation in the time tag
/// stream.
///
/// Each TT event has an associated channel that has a number (1-18) and can
/// be either positive, if events are detected in the rising edge, or negative
/// if they're detected on the falling edge. Additionally each active channel
/// has a threshold value.
///
/// This function converts the user's choice into the internal representation
/// detailed above. An empty channel is given the value 0.
fn convert_user_channel_input_to_num(channel: (ChannelNumber, EdgeDetected, f32)) -> InputChannel {
    let edge: i32 = match channel.1 {
        EdgeDetected::Rising => 1,
        EdgeDetected::Falling => -1,
    };
    let ch = edge
        * match channel.0 {
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
            ChannelNumber::Ignore => 0,
            ChannelNumber::Disconnected => 0,
        };
    InputChannel::new(ch, channel.2)
}

#[derive(Clone)]
pub struct AppConfigBuilder {
    filename: String,
    point_color: Point3<f32>,
    ignored_channels: Vec<InputChannel>,
    rows: u32,
    columns: u32,
    planes: u32,
    scan_period: Period,
    tag_period: Period,
    bidir: Bidirectionality,
    fill_fraction: f32, // (0..100)
    frame_dead_time: Picosecond,
    replay_existing: bool,
    rolling_avg: u16,
    line_shift: Picosecond,
    pmt1_ch: InputChannel,
    pmt2_ch: InputChannel,
    pmt3_ch: InputChannel,
    pmt4_ch: InputChannel,
    laser_ch: InputChannel,
    frame_ch: InputChannel,
    line_ch: InputChannel,
    taglens_ch: InputChannel,
}

impl AppConfigBuilder {
    /// Generate an instance with default values. Useful mainly for quick
    /// testing.
    pub fn default() -> AppConfigBuilder {
        AppConfigBuilder {
            filename: "target/test.npy".to_string(),
            point_color: Point3::new(1.0f32, 1.0, 1.0),
            ignored_channels: vec![],
            rows: 256,
            columns: 256,
            planes: 10,
            scan_period: Period::from_freq(7923.0),
            tag_period: Period::from_freq(189800.0),
            bidir: Bidirectionality::Bidir,
            replay_existing: false,
            rolling_avg: 1,
            fill_fraction: 71.0,
            frame_dead_time: 1_310_000_000,
            line_shift: 0,
            pmt1_ch: InputChannel::new(1, 0.0),
            pmt2_ch: InputChannel::new(0, 0.0),
            pmt3_ch: InputChannel::new(0, 0.0),
            pmt4_ch: InputChannel::new(0, 0.0),
            laser_ch: InputChannel::new(0, 0.0),
            frame_ch: InputChannel::new(0, 0.0),
            line_ch: InputChannel::new(-2, 0.0),
            taglens_ch: InputChannel::new(3, 0.0),
        }
    }

    pub fn build(&self) -> AppConfig {
        AppConfig {
            filename: self.filename.clone(),
            point_color: self.point_color,
            ignored_channels: self.ignored_channels.clone(),
            rows: self.rows,
            columns: self.columns,
            planes: self.planes,
            scan_period: self.scan_period,
            tag_period: self.tag_period,
            bidir: self.bidir,
            rolling_avg: self.rolling_avg,
            fill_fraction: self.fill_fraction,
            frame_dead_time: self.frame_dead_time,
            pmt1_ch: self.pmt1_ch,
            pmt2_ch: self.pmt2_ch,
            pmt3_ch: self.pmt3_ch,
            pmt4_ch: self.pmt4_ch,
            laser_ch: self.laser_ch,
            frame_ch: self.frame_ch,
            line_ch: self.line_ch,
            taglens_ch: self.taglens_ch,
            replay_existing: self.replay_existing,
            line_shift: self.line_shift,
        }
    }

    pub fn with_filename(&mut self, filename: String) -> &mut Self {
        self.filename = filename;
        self
    }

    pub fn with_point_color(&mut self, point_color: Point3<f32>) -> &mut Self {
        self.point_color = point_color;
        self
    }

    pub fn with_rows(&mut self, rows: u32) -> &mut Self {
        assert!(rows < 100_000);
        self.rows = rows;
        self
    }

    pub fn with_columns(&mut self, columns: u32) -> &mut Self {
        assert!(columns < 100_000);
        self.columns = columns;
        self
    }

    pub fn with_planes(&mut self, planes: u32) -> &mut Self {
        assert!(planes < 100_000);
        self.planes = planes;
        self
    }

    pub fn with_scan_period(&mut self, scan_period: Period) -> &mut Self {
        self.scan_period = scan_period;
        self
    }

    pub fn with_tag_period(&mut self, tag_period: Period) -> &mut Self {
        assert!(*tag_period > 1_000_000);
        self.tag_period = tag_period;
        self
    }

    pub fn with_bidir<T: Into<Bidirectionality>>(&mut self, bidir: T) -> &mut Self {
        self.bidir = bidir.into();
        self
    }

    pub fn with_rolling_avg(&mut self, rolling_avg: u16) -> &mut Self {
        self.rolling_avg = rolling_avg;
        self
    }

    pub fn with_fill_fraction<T: Into<f32>>(&mut self, fill_fraction: T) -> &mut Self {
        let fill_fraction = fill_fraction.into();
        assert!((0.0..=100.0).contains(&fill_fraction));
        self.fill_fraction = fill_fraction;
        self
    }

    pub fn with_frame_dead_time(&mut self, frame_dead_time: Picosecond) -> &mut Self {
        assert!((0..=10_000_000_000_000).contains(&frame_dead_time));
        self.frame_dead_time = frame_dead_time;
        self
    }

    pub fn with_pmt1_ch(&mut self, pmt1_ch: InputChannel) -> &mut Self {
        assert!(pmt1_ch.channel.abs() <= MAX_TIMETAGGER_INPUTS);
        self.pmt1_ch = pmt1_ch;
        self
    }

    pub fn with_pmt2_ch(&mut self, pmt2_ch: InputChannel) -> &mut Self {
        assert!(pmt2_ch.channel.abs() <= MAX_TIMETAGGER_INPUTS);
        self.pmt2_ch = pmt2_ch;
        self
    }

    pub fn with_pmt3_ch(&mut self, pmt3_ch: InputChannel) -> &mut Self {
        assert!(pmt3_ch.channel.abs() <= MAX_TIMETAGGER_INPUTS);
        self.pmt3_ch = pmt3_ch;
        self
    }

    pub fn with_pmt4_ch(&mut self, pmt4_ch: InputChannel) -> &mut Self {
        assert!(pmt4_ch.channel.abs() <= MAX_TIMETAGGER_INPUTS);
        self.pmt4_ch = pmt4_ch;
        self
    }

    pub fn with_laser_ch(&mut self, laser_ch: InputChannel) -> &mut Self {
        assert!(laser_ch.channel.abs() <= MAX_TIMETAGGER_INPUTS);
        self.laser_ch = laser_ch;
        self
    }

    pub fn with_frame_ch(&mut self, frame_ch: InputChannel) -> &mut Self {
        assert!(frame_ch.channel.abs() <= MAX_TIMETAGGER_INPUTS);
        self.frame_ch = frame_ch;
        self
    }

    pub fn with_line_ch(&mut self, line_ch: InputChannel) -> &mut Self {
        assert!(line_ch.channel.abs() <= MAX_TIMETAGGER_INPUTS);
        self.line_ch = line_ch;
        self
    }

    pub fn with_taglens_ch(&mut self, taglens_ch: InputChannel) -> &mut Self {
        assert!(taglens_ch.channel.abs() <= MAX_TIMETAGGER_INPUTS);
        self.taglens_ch = taglens_ch;
        self
    }

    pub fn with_replay_existing(&mut self, replay_existing: bool) -> &mut Self {
        self.replay_existing = replay_existing;
        self
    }

    pub fn with_ignored_channels(&mut self, ignored_channels: Vec<InputChannel>) -> &mut Self {
        self.ignored_channels = ignored_channels;
        self
    }

    pub fn with_line_shift(&mut self, line_shift: Picosecond) -> &mut Self {
        self.line_shift = line_shift;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper method to test config-dependent things without actually caring
    /// about the different config values
    fn setup_default_config() -> AppConfigBuilder {
        AppConfigBuilder::default()
            .with_point_color(Point3::new(1.0f32, 1.0, 1.0))
            .with_rows(256)
            .with_columns(256)
            .with_planes(10)
            .with_scan_period(Period::from_freq(7926.17))
            .with_tag_period(Period::from_freq(189800))
            .with_bidir(Bidirectionality::Bidir)
            .with_rolling_avg(1)
            .with_fill_fraction(71.3)
            .with_frame_dead_time(8 * *Period::from_freq(7926.17))
            .with_pmt1_ch(InputChannel::new(-1, 0.0))
            .with_pmt2_ch(InputChannel::new(0, 0.0))
            .with_pmt3_ch(InputChannel::new(0, 0.0))
            .with_pmt4_ch(InputChannel::new(0, 0.0))
            .with_laser_ch(InputChannel::new(0, 0.0))
            .with_frame_ch(InputChannel::new(0, 0.0))
            .with_line_ch(InputChannel::new(2, 0.0))
            .with_taglens_ch(InputChannel::new(3, 0.0))
            .with_replay_existing(false)
            .with_ignored_channels(vec![])
            .clone()
    }

    #[test]
    fn inputs_indexing_positive() {
        let config = setup_default_config()
            .with_pmt1_ch(InputChannel::new(1, 0.0))
            .with_pmt2_ch(InputChannel::new(2, 0.0))
            .with_pmt3_ch(InputChannel::new(3, 0.0))
            .with_pmt4_ch(InputChannel::new(4, 0.0))
            .with_laser_ch(InputChannel::new(5, 0.0))
            .with_frame_ch(InputChannel::new(6, 0.0))
            .with_line_ch(InputChannel::new(7, 0.0))
            .with_taglens_ch(InputChannel::new(8, 0.0))
            .build();
        let inputs = Inputs::from_config(&config);
        assert_eq!(inputs[1], DataType::Pmt1);
    }

    #[test]
    fn inputs_indexing_positive_edge() {
        let config = setup_default_config()
            .with_pmt1_ch(InputChannel::new(1, 0.0))
            .with_pmt2_ch(InputChannel::new(2, 0.0))
            .with_pmt3_ch(InputChannel::new(3, 0.0))
            .with_pmt4_ch(InputChannel::new(4, 0.0))
            .with_laser_ch(InputChannel::new(5, 0.0))
            .with_frame_ch(InputChannel::new(6, 0.0))
            .with_line_ch(InputChannel::new(7, 0.0))
            .with_taglens_ch(InputChannel::new(18, 0.0))
            .build();
        let inputs = Inputs::from_config(&config);
        assert_eq!(inputs[18], DataType::TagLens);
    }

    #[test]
    fn inputs_indexing_negative() {
        let config = setup_default_config()
            .with_pmt1_ch(InputChannel::new(-1, 0.0))
            .with_pmt2_ch(InputChannel::new(2, 0.0))
            .with_pmt3_ch(InputChannel::new(3, 0.0))
            .with_pmt4_ch(InputChannel::new(4, 0.0))
            .with_laser_ch(InputChannel::new(5, 0.0))
            .with_frame_ch(InputChannel::new(6, 0.0))
            .with_line_ch(InputChannel::new(7, 0.0))
            .with_taglens_ch(InputChannel::new(8, 0.0))
            .build();
        let inputs = Inputs::from_config(&config);
        assert_eq!(inputs[-1], DataType::Pmt1);
    }

    #[test]
    fn inputs_indexing_negative_edge() {
        let config = setup_default_config()
            .with_pmt1_ch(InputChannel::new(-1, 0.0))
            .with_pmt2_ch(InputChannel::new(2, 0.0))
            .with_pmt3_ch(InputChannel::new(3, 0.0))
            .with_pmt4_ch(InputChannel::new(4, 0.0))
            .with_laser_ch(InputChannel::new(5, 0.0))
            .with_frame_ch(InputChannel::new(6, 0.0))
            .with_line_ch(InputChannel::new(7, 0.0))
            .with_taglens_ch(InputChannel::new(-18, 0.0))
            .build();
        let inputs = Inputs::from_config(&config);
        assert_eq!(inputs[-18], DataType::TagLens);
    }

    #[test]
    #[should_panic(expected = "One of the channels was a duplicate")]
    fn inputs_duplicate_channel() {
        let config = setup_default_config()
            .with_pmt1_ch(InputChannel::new(-1, 0.0))
            .with_pmt2_ch(InputChannel::new(-1, 0.0))
            .build();
        let _ = Inputs::from_config(&config);
    }

    #[test]
    fn inputs_not_all_channels_are_used() {
        let config = setup_default_config()
            .with_pmt1_ch(InputChannel::new(-1, 0.0))
            .with_pmt2_ch(InputChannel::new(2, 0.0))
            .with_pmt3_ch(InputChannel::new(3, 0.0))
            .with_pmt4_ch(InputChannel::new(4, 0.0))
            .with_laser_ch(InputChannel::new(0, 0.0))
            .with_frame_ch(InputChannel::new(0, 0.0))
            .with_line_ch(InputChannel::new(0, 0.0))
            .with_taglens_ch(InputChannel::new(0, 0.0))
            .build();
        let _ = Inputs::from_config(&config);
    }

    #[test]
    fn frame_time_bidir() {
        let config = setup_default_config().with_bidir(true).build();
        assert_eq!(config.calc_frame_duration(), 16_149_035_264i64);
    }

    #[test]
    fn frame_time_unidir() {
        let config = setup_default_config().with_bidir(false).build();
        assert_eq!(config.calc_frame_duration(), 32_298_070_784i64);
    }

    #[test]
    fn frame_rate_unidir() {
        let config = setup_default_config().with_bidir(false).build();
        assert_eq!(config.frame_rate(), 30.961601f32);
    }

    #[test]
    fn frame_rate_bidir() {
        let config = setup_default_config().with_bidir(true).build();
        assert_eq!(config.frame_rate(), 61.923203f32);
    }

    #[test]
    fn string_ms_to_ps_simple() {
        let deadtime = "1.0";
        assert_eq!(1_000_000_000, string_ms_to_ps(deadtime).unwrap());
    }

    #[test]
    fn string_ms_to_ps_complex() {
        let deadtime = "2.009";
        assert_eq!(2_009_000_000, string_ms_to_ps(deadtime).unwrap());
    }

    #[test]
    fn channel_inp_to_num_disconneted_positive() {
        let result = convert_user_channel_input_to_num((
            ChannelNumber::Disconnected,
            EdgeDetected::Rising,
            0.0,
        ));
        assert_eq!(result, InputChannel::new(0, 0.0));
    }

    #[test]
    fn channel_inp_to_num_disconneted_negative() {
        let result = convert_user_channel_input_to_num((
            ChannelNumber::Disconnected,
            EdgeDetected::Falling,
            0.0,
        ));
        assert_eq!(result, InputChannel::new(0, 0.0));
    }

    #[test]
    fn channel_inp_to_num_standard_falling() {
        let result = convert_user_channel_input_to_num((
            ChannelNumber::Channel3,
            EdgeDetected::Falling,
            -1.0,
        ));
        assert_eq!(result, InputChannel::new(-3, -1.0));
    }

    #[test]
    fn channel_inp_to_num_standard_rising() {
        let result =
            convert_user_channel_input_to_num((ChannelNumber::Channel3, EdgeDetected::Rising, 1.0));
        assert_eq!(result, InputChannel::new(3, 1.0));
    }

    #[test]
    fn ignored_to_vec_standard() {
        let test = "1,2, -3, -4";
        let ret = convert_ignored_to_vec(test);
        assert_eq!(
            ret,
            vec![1i32, 2, -3, -4]
                .iter()
                .map(|ch| InputChannel::new(*ch, 0.0))
                .collect::<Vec<InputChannel>>()
        );
    }

    #[test]
    fn ignored_to_vec_single() {
        let test1 = "1";
        let test2 = "1,";
        let ret1 = convert_ignored_to_vec(test1);
        let ret2 = convert_ignored_to_vec(test2);
        assert_eq!(ret1, vec![InputChannel::new(1, 0.0)]);
        assert_eq!(ret2, vec![InputChannel::new(1, 0.0)]);
    }

    #[test]
    fn ignored_to_vec_empty() {
        let test = "";
        let ret = convert_ignored_to_vec(test);
        assert_eq!(ret, Vec::<InputChannel>::new());
    }
}
