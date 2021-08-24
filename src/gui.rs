use std::fmt::Write;
use std::path::PathBuf;

use iced::{
    button, pick_list, text_input, Align, Application, Button, Checkbox, Clipboard, Column,
    Command, Container, Element, Image, Length, PickList, Row, Text, TextInput,
};
use serde::{Deserialize, Serialize};

use crate::{channel_value_to_pair, start_acquisition, DEFAULT_CONFIG_FNAME};
use crate::{
    configuration::{AppConfig, InputChannel},
    snakes::Picosecond,
};

#[derive(Default)]
pub struct MainAppGui {
    filename_input: text_input::State,
    filename_value: String,
    rows_input: text_input::State,
    rows_value: String,
    columns_input: text_input::State,
    columns_value: String,
    planes_input: text_input::State,
    planes_value: String,
    scan_period_input: text_input::State,
    scan_period_value: String,
    tag_period_input: text_input::State,
    tag_period_value: String,
    bidirectional: bool,
    rolling_avg_input: text_input::State,
    rolling_avg_value: String,
    fill_fraction_input: text_input::State,
    fill_fraction_value: String,
    frame_dead_time_input: text_input::State,
    frame_dead_time_value: String,
    pmt1_pick_list: pick_list::State<ChannelNumber>,
    pmt1_selected: ChannelNumber,
    pmt1_edge_list: pick_list::State<EdgeDetected>,
    pmt1_edge_selected: EdgeDetected,
    pmt1_threshold_input: text_input::State,
    pmt1_threshold_value: String,
    pmt2_pick_list: pick_list::State<ChannelNumber>,
    pmt2_selected: ChannelNumber,
    pmt2_edge_list: pick_list::State<EdgeDetected>,
    pmt2_edge_selected: EdgeDetected,
    pmt2_threshold_input: text_input::State,
    pmt2_threshold_value: String,
    pmt3_pick_list: pick_list::State<ChannelNumber>,
    pmt3_selected: ChannelNumber,
    pmt3_edge_list: pick_list::State<EdgeDetected>,
    pmt3_edge_selected: EdgeDetected,
    pmt3_threshold_input: text_input::State,
    pmt3_threshold_value: String,
    pmt4_pick_list: pick_list::State<ChannelNumber>,
    pmt4_selected: ChannelNumber,
    pmt4_edge_list: pick_list::State<EdgeDetected>,
    pmt4_edge_selected: EdgeDetected,
    pmt4_threshold_input: text_input::State,
    pmt4_threshold_value: String,
    laser_pick_list: pick_list::State<ChannelNumber>,
    laser_selected: ChannelNumber,
    laser_edge_list: pick_list::State<EdgeDetected>,
    laser_edge_selected: EdgeDetected,
    laser_threshold_input: text_input::State,
    laser_threshold_value: String,
    frame_pick_list: pick_list::State<ChannelNumber>,
    frame_selected: ChannelNumber,
    frame_edge_list: pick_list::State<EdgeDetected>,
    frame_edge_selected: EdgeDetected,
    frame_threshold_input: text_input::State,
    frame_threshold_value: String,
    line_pick_list: pick_list::State<ChannelNumber>,
    line_selected: ChannelNumber,
    line_edge_list: pick_list::State<EdgeDetected>,
    line_edge_selected: EdgeDetected,
    line_threshold_input: text_input::State,
    line_threshold_value: String,
    taglens_pick_list: pick_list::State<ChannelNumber>,
    taglens_selected: ChannelNumber,
    taglens_edge_list: pick_list::State<EdgeDetected>,
    taglens_edge_selected: EdgeDetected,
    taglens_threshold_input: text_input::State,
    taglens_threshold_value: String,
    replay_existing: bool,
    ignored_channels_input: text_input::State,
    ignored_channels_value: String,
    line_shift_input: text_input::State,
    line_shift_value: String,
    run_button: button::State,
}

impl MainAppGui {
    pub(crate) fn get_filename(&self) -> &str {
        &self.filename_value
    }

    pub(crate) fn get_num_rows(&self) -> &str {
        &self.rows_value
    }

    pub(crate) fn get_num_columns(&self) -> &str {
        &self.columns_value
    }

    pub(crate) fn get_num_planes(&self) -> &str {
        &self.planes_value
    }

    pub(crate) fn get_scan_period(&self) -> &str {
        &self.scan_period_value
    }

    pub(crate) fn get_taglens_period(&self) -> &str {
        &self.tag_period_value
    }

    pub(crate) fn get_bidirectionality(&self) -> bool {
        self.bidirectional
    }

    pub(crate) fn get_frame_dead_time(&self) -> &str {
        &self.frame_dead_time_value
    }

    pub(crate) fn get_fill_fraction(&self) -> &str {
        &self.fill_fraction_value
    }

    pub(crate) fn get_pmt1_channel(&self) -> (ChannelNumber, EdgeDetected, f32) {
        (
            self.pmt1_selected,
            self.pmt1_edge_selected,
            self.pmt1_threshold_value.parse::<f32>().unwrap_or(0.0),
        )
    }

    pub(crate) fn get_pmt2_channel(&self) -> (ChannelNumber, EdgeDetected, f32) {
        (
            self.pmt2_selected,
            self.pmt2_edge_selected,
            self.pmt2_threshold_value.parse::<f32>().unwrap_or(0.0),
        )
    }

    pub(crate) fn get_pmt3_channel(&self) -> (ChannelNumber, EdgeDetected, f32) {
        (
            self.pmt3_selected,
            self.pmt3_edge_selected,
            self.pmt3_threshold_value.parse::<f32>().unwrap_or(0.0),
        )
    }

    pub(crate) fn get_pmt4_channel(&self) -> (ChannelNumber, EdgeDetected, f32) {
        (
            self.pmt4_selected,
            self.pmt4_edge_selected,
            self.pmt4_threshold_value.parse::<f32>().unwrap_or(0.0),
        )
    }

    pub(crate) fn get_laser_channel(&self) -> (ChannelNumber, EdgeDetected, f32) {
        (
            self.laser_selected,
            self.laser_edge_selected,
            self.laser_threshold_value.parse::<f32>().unwrap_or(0.0),
        )
    }

    pub(crate) fn get_frame_channel(&self) -> (ChannelNumber, EdgeDetected, f32) {
        (
            self.frame_selected,
            self.frame_edge_selected,
            self.frame_threshold_value.parse::<f32>().unwrap_or(0.0),
        )
    }

    pub(crate) fn get_line_channel(&self) -> (ChannelNumber, EdgeDetected, f32) {
        (
            self.line_selected,
            self.line_edge_selected,
            self.line_threshold_value.parse::<f32>().unwrap_or(0.0),
        )
    }

    pub(crate) fn get_tag_channel(&self) -> (ChannelNumber, EdgeDetected, f32) {
        (
            self.taglens_selected,
            self.taglens_edge_selected,
            self.taglens_threshold_value.parse::<f32>().unwrap_or(0.0),
        )
    }

    pub(crate) fn get_replay_existing(&self) -> bool {
        self.replay_existing
    }

    pub(crate) fn get_ignored_channels(&self) -> &str {
        &self.ignored_channels_value
    }

    pub(crate) fn get_line_shift(&self) -> &str {
        &self.line_shift_value
    }

    pub(crate) fn get_rolling_avg(&self) -> u16 {
        self.rolling_avg_value.parse::<u16>().unwrap_or(1)
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    FilenameChanged(String),
    RowsChanged(String),
    ColumnsChanged(String),
    PlanesChanged(String),
    ScanPeriodChanged(String),
    TagLensPeriodChanged(String),
    BidirectionalityChanged(bool),
    FillFractionChanged(String),
    FrameDeadTimeChanged(String),
    Pmt1Changed(ChannelNumber),
    Pmt1EdgeChanged(EdgeDetected),
    Pmt1ThresholdChanged(String),
    Pmt2Changed(ChannelNumber),
    Pmt2EdgeChanged(EdgeDetected),
    Pmt2ThresholdChanged(String),
    Pmt3Changed(ChannelNumber),
    Pmt3EdgeChanged(EdgeDetected),
    Pmt3ThresholdChanged(String),
    Pmt4Changed(ChannelNumber),
    Pmt4EdgeChanged(EdgeDetected),
    Pmt4ThresholdChanged(String),
    LaserChanged(ChannelNumber),
    LaserEdgeChanged(EdgeDetected),
    LaserThresholdChanged(String),
    FrameChanged(ChannelNumber),
    FrameEdgeChanged(EdgeDetected),
    FrameThresholdChanged(String),
    LineChanged(ChannelNumber),
    LineEdgeChanged(EdgeDetected),
    LineThresholdChanged(String),
    TagLensChanged(ChannelNumber),
    TagLensEdgeChanged(EdgeDetected),
    TagLensThresholdChanged(String),
    ReplayExistingChanged(bool),
    IgnoredChannelsChanged(String),
    LineShiftChanged(String),
    RollingAvgChanged(String),
    ButtonPressed,
    StartedAcquistion(()),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChannelNumber {
    Channel1,
    Channel2,
    Channel3,
    Channel4,
    Channel5,
    Channel6,
    Channel7,
    Channel8,
    Channel9,
    Channel10,
    Channel11,
    Channel12,
    Channel13,
    Channel14,
    Channel15,
    Channel16,
    Channel17,
    Channel18,
    /// This channel has an input but we wish to currently discard it
    Ignore,
    Disconnected,
}

impl std::fmt::Display for ChannelNumber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                ChannelNumber::Disconnected => "Disconnected",
                ChannelNumber::Ignore => "Ignore",
                ChannelNumber::Channel1 => "Channel 1",
                ChannelNumber::Channel2 => "Channel 2",
                ChannelNumber::Channel3 => "Channel 3",
                ChannelNumber::Channel4 => "Channel 4",
                ChannelNumber::Channel5 => "Channel 5",
                ChannelNumber::Channel6 => "Channel 6",
                ChannelNumber::Channel7 => "Channel 7",
                ChannelNumber::Channel8 => "Channel 8",
                ChannelNumber::Channel9 => "Channel 9",
                ChannelNumber::Channel10 => "Channel 10",
                ChannelNumber::Channel11 => "Channel 11",
                ChannelNumber::Channel12 => "Channel 12",
                ChannelNumber::Channel13 => "Channel 13",
                ChannelNumber::Channel14 => "Channel 14",
                ChannelNumber::Channel15 => "Channel 15",
                ChannelNumber::Channel16 => "Channel 16",
                ChannelNumber::Channel17 => "Channel 17",
                ChannelNumber::Channel18 => "Channel 18",
            }
        )
    }
}

impl ChannelNumber {
    const ALL: [ChannelNumber; 20] = [
        ChannelNumber::Disconnected,
        ChannelNumber::Channel1,
        ChannelNumber::Channel2,
        ChannelNumber::Channel3,
        ChannelNumber::Channel4,
        ChannelNumber::Channel5,
        ChannelNumber::Channel6,
        ChannelNumber::Channel7,
        ChannelNumber::Channel8,
        ChannelNumber::Channel9,
        ChannelNumber::Channel10,
        ChannelNumber::Channel11,
        ChannelNumber::Channel12,
        ChannelNumber::Channel13,
        ChannelNumber::Channel14,
        ChannelNumber::Channel15,
        ChannelNumber::Channel16,
        ChannelNumber::Channel17,
        ChannelNumber::Channel18,
        ChannelNumber::Ignore,
    ];
}

impl Default for ChannelNumber {
    fn default() -> Self {
        ChannelNumber::Disconnected
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeDetected {
    Rising,
    Falling,
}

impl EdgeDetected {
    const ALL: [EdgeDetected; 2] = [EdgeDetected::Rising, EdgeDetected::Falling];
}

impl Default for EdgeDetected {
    fn default() -> Self {
        EdgeDetected::Rising
    }
}

impl std::fmt::Display for EdgeDetected {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                EdgeDetected::Rising => "Rising",
                EdgeDetected::Falling => "Falling",
            }
        )
    }
}

/// Separate the ignored channels into a string of comma-separated numbers
fn vec_to_comma_sep_string(a: &[InputChannel]) -> String {
    let mut f = a.iter().fold(String::new(), |mut s, &n| {
        write!(s, "{},", n.channel).ok();
        s
    });
    f.pop();
    f
}

/// Converts the given picoseconds value to a miliseconds one.
///
/// Used in the GUI, when converting the interal representation of the frame
/// dead time from ps to ms, which is displayed to the user.
fn ps_to_ms(time: Picosecond) -> f32 {
    (time as f64 / 1_000_000_000.0f64) as f32
}

impl Application for MainAppGui {
    type Executor = iced::executor::Default;
    type Message = Message;
    type Flags = AppConfig;

    /// Create a new MainAppGui with values taken from the given config.
    ///
    /// The app is created in a default state, which helps with the
    /// initialization of the buttons and such, and then the individual fields
    /// are updated from the input config instance.
    fn new(prev_config: AppConfig) -> (MainAppGui, Command<Message>) {
        let mut app = MainAppGui {
            filename_value: prev_config.filename,
            rows_value: prev_config.rows.to_string(),
            columns_value: prev_config.columns.to_string(),
            planes_value: prev_config.planes.to_string(),
            scan_period_value: prev_config.scan_period.to_hz().to_string(),
            tag_period_value: prev_config.tag_period.to_hz().to_string(),
            bidirectional: prev_config.bidir.into(),
            fill_fraction_value: prev_config.fill_fraction.to_string(),
            frame_dead_time_value: ps_to_ms(prev_config.frame_dead_time).to_string(),
            replay_existing: prev_config.replay_existing,
            ignored_channels_value: vec_to_comma_sep_string(&prev_config.ignored_channels),
            line_shift_value: prev_config.line_shift.to_string(),
            ..Default::default()
        };
        let pmt1 = channel_value_to_pair(prev_config.pmt1_ch);
        app.pmt1_selected = pmt1.0;
        app.pmt1_edge_selected = pmt1.1;
        app.pmt1_threshold_value = pmt1.2.to_string();
        let pmt2 = channel_value_to_pair(prev_config.pmt2_ch);
        app.pmt2_selected = pmt2.0;
        app.pmt2_edge_selected = pmt2.1;
        app.pmt2_threshold_value = pmt2.2.to_string();
        let pmt3 = channel_value_to_pair(prev_config.pmt3_ch);
        app.pmt3_selected = pmt3.0;
        app.pmt3_edge_selected = pmt3.1;
        app.pmt3_threshold_value = pmt3.2.to_string();
        let pmt4 = channel_value_to_pair(prev_config.pmt4_ch);
        app.pmt4_selected = pmt4.0;
        app.pmt4_edge_selected = pmt4.1;
        app.pmt4_threshold_value = pmt4.2.to_string();
        let laser = channel_value_to_pair(prev_config.laser_ch);
        app.laser_selected = laser.0;
        app.laser_edge_selected = laser.1;
        app.laser_threshold_value = laser.2.to_string();
        let frame = channel_value_to_pair(prev_config.frame_ch);
        app.frame_selected = frame.0;
        app.frame_edge_selected = frame.1;
        app.frame_threshold_value = frame.2.to_string();
        let line = channel_value_to_pair(prev_config.line_ch);
        app.line_selected = line.0;
        app.line_edge_selected = line.1;
        app.line_threshold_value = line.2.to_string();
        let taglens = channel_value_to_pair(prev_config.taglens_ch);
        app.taglens_selected = taglens.0;
        app.taglens_edge_selected = taglens.1;
        app.taglens_threshold_value = taglens.2.to_string();

        (app, Command::none())
    }

    fn title(&self) -> String {
        String::from("rPySight 0.1.0")
    }

    fn update(&mut self, message: Message, _clip: &mut Clipboard) -> Command<Self::Message> {
        match message {
            Message::FilenameChanged(filename) => {
                self.filename_value = filename;
                Command::none()
            }
            Message::RowsChanged(rows) => {
                self.rows_value = rows;
                Command::none()
            }
            Message::ColumnsChanged(columns) => {
                self.columns_value = columns;
                Command::none()
            }
            Message::PlanesChanged(planes) => {
                self.planes_value = planes;
                Command::none()
            }
            Message::ScanPeriodChanged(period) => {
                self.scan_period_value = period;
                Command::none()
            }
            Message::TagLensPeriodChanged(period) => {
                self.tag_period_value = period;
                Command::none()
            }
            Message::BidirectionalityChanged(bidir) => {
                self.bidirectional = bidir;
                Command::none()
            }
            Message::FillFractionChanged(fillfrac) => {
                self.fill_fraction_value = fillfrac;
                Command::none()
            }
            Message::FrameDeadTimeChanged(deadtime) => {
                self.frame_dead_time_value = deadtime;
                Command::none()
            }
            Message::Pmt1Changed(pmt1) => {
                self.pmt1_selected = pmt1;
                Command::none()
            }
            Message::Pmt1EdgeChanged(pmt1_edge) => {
                self.pmt1_edge_selected = pmt1_edge;
                Command::none()
            }
            Message::Pmt1ThresholdChanged(pmt1_thresh) => {
                self.pmt1_threshold_value = pmt1_thresh;
                Command::none()
            }
            Message::Pmt2Changed(pmt2) => {
                self.pmt2_selected = pmt2;
                Command::none()
            }
            Message::Pmt2EdgeChanged(pmt2_edge) => {
                self.pmt2_edge_selected = pmt2_edge;
                Command::none()
            }
            Message::Pmt2ThresholdChanged(pmt2_thresh) => {
                self.pmt2_threshold_value = pmt2_thresh;
                Command::none()
            }
            Message::Pmt3Changed(pmt3) => {
                self.pmt3_selected = pmt3;
                Command::none()
            }
            Message::Pmt3EdgeChanged(pmt3_edge) => {
                self.pmt3_edge_selected = pmt3_edge;
                Command::none()
            }
            Message::Pmt3ThresholdChanged(pmt3_thresh) => {
                self.pmt3_threshold_value = pmt3_thresh;
                Command::none()
            }
            Message::Pmt4Changed(pmt4) => {
                self.pmt4_selected = pmt4;
                Command::none()
            }
            Message::Pmt4EdgeChanged(pmt4_edge) => {
                self.pmt4_edge_selected = pmt4_edge;
                Command::none()
            }
            Message::Pmt4ThresholdChanged(pmt4_thresh) => {
                self.pmt4_threshold_value = pmt4_thresh;
                Command::none()
            }
            Message::LaserChanged(laser) => {
                self.laser_selected = laser;
                Command::none()
            }
            Message::LaserEdgeChanged(laser_edge) => {
                self.laser_edge_selected = laser_edge;
                Command::none()
            }
            Message::LaserThresholdChanged(laser_thresh) => {
                self.laser_threshold_value = laser_thresh;
                Command::none()
            }
            Message::FrameChanged(frame) => {
                self.frame_selected = frame;
                Command::none()
            }
            Message::FrameEdgeChanged(frame_edge) => {
                self.frame_edge_selected = frame_edge;
                Command::none()
            }
            Message::FrameThresholdChanged(frame_thresh) => {
                self.frame_threshold_value = frame_thresh;
                Command::none()
            }
            Message::LineChanged(line) => {
                self.line_selected = line;
                Command::none()
            }
            Message::LineEdgeChanged(line_edge) => {
                self.line_edge_selected = line_edge;
                Command::none()
            }
            Message::LineThresholdChanged(line_thresh) => {
                self.line_threshold_value = line_thresh;
                Command::none()
            }
            Message::TagLensChanged(taglens) => {
                self.taglens_selected = taglens;
                Command::none()
            }
            Message::TagLensEdgeChanged(taglens_edge) => {
                self.taglens_edge_selected = taglens_edge;
                Command::none()
            }
            Message::TagLensThresholdChanged(taglens_thresh) => {
                self.taglens_threshold_value = taglens_thresh;
                Command::none()
            }
            Message::ReplayExistingChanged(replay_existing) => {
                self.replay_existing = replay_existing;
                Command::none()
            }
            Message::IgnoredChannelsChanged(ignored_str) => {
                self.ignored_channels_value = ignored_str;
                Command::none()
            }
            Message::LineShiftChanged(line_shift) => {
                self.line_shift_value = line_shift;
                Command::none()
            }
            Message::RollingAvgChanged(rolling_avg) => {
                self.rolling_avg_value = rolling_avg;
                Command::none()
            }
            Message::ButtonPressed => Command::perform(
                start_acquisition(
                    PathBuf::from(DEFAULT_CONFIG_FNAME),
                    AppConfig::from_user_input(self).expect(""),
                ),
                Message::StartedAcquistion,
            ),
            Message::StartedAcquistion(()) => Command::none(),
        }
    }

    fn view(&mut self) -> Element<Message> {
        let filename = TextInput::new(
            &mut self.filename_input,
            "Save to",
            &self.filename_value,
            Message::FilenameChanged,
        )
        .padding(10)
        .size(20);
        let filename_label =
            Text::new("Filename").vertical_alignment(iced::VerticalAlignment::Bottom);
        let replay_existing_checkbox = Checkbox::new(
            self.replay_existing,
            "Replay existing?",
            Message::ReplayExistingChanged,
        )
        .size(20);
        let filename_row = Row::new()
            .spacing(10)
            .align_items(Align::Center)
            .push(filename_label)
            .push(filename)
            .push(replay_existing_checkbox);

        let rows = TextInput::new(
            &mut self.rows_input,
            "Rows [px]",
            &self.rows_value,
            Message::RowsChanged,
        )
        .padding(10)
        .size(20);
        let rows_label = Text::new("Rows");
        let rows_row = Row::new()
            .spacing(10)
            .align_items(Align::Center)
            .push(rows_label)
            .push(rows);

        let columns = TextInput::new(
            &mut self.columns_input,
            "Columns [px]",
            &self.columns_value,
            Message::ColumnsChanged,
        )
        .padding(10)
        .size(20);
        let columns_label = Text::new("Columns");
        let columns_row = Row::new()
            .spacing(10)
            .align_items(Align::Center)
            .push(columns_label)
            .push(columns);

        let planes = TextInput::new(
            &mut self.planes_input,
            "Planes [px] (1 for planar imaging)",
            &self.planes_value,
            Message::PlanesChanged,
        )
        .padding(10)
        .size(20);
        let planes_label = Text::new("Planes");
        let planes_row = Row::new()
            .spacing(10)
            .align_items(Align::Center)
            .push(planes_label)
            .push(planes);

        let scan_period = TextInput::new(
            &mut self.scan_period_input,
            "Scan Frequency [Hz]",
            &self.scan_period_value,
            Message::ScanPeriodChanged,
        )
        .padding(10)
        .size(20);
        let scan_period_label = Text::new("Scan Frequency");
        let scan_period_row = Row::new()
            .spacing(10)
            .align_items(Align::Center)
            .push(scan_period_label)
            .push(scan_period);

        let taglens_period = TextInput::new(
            &mut self.tag_period_input,
            "TAG Lens Frequency [Hz]",
            &self.tag_period_value,
            Message::TagLensPeriodChanged,
        )
        .padding(10)
        .size(20);
        let taglens_period_label = Text::new("TAG Lens Frequency");
        let taglens_period_row = Row::new()
            .spacing(10)
            .align_items(Align::Center)
            .push(taglens_period_label)
            .push(taglens_period);

        let fillfrac = TextInput::new(
            &mut self.fill_fraction_input,
            "Fill Fraction [%]",
            &self.fill_fraction_value,
            Message::FillFractionChanged,
        )
        .padding(10)
        .size(20);
        let fillfrac_label = Text::new("Fill Fraction");
        let fillfrac_row = Row::new()
            .spacing(10)
            .align_items(Align::Center)
            .push(fillfrac_label)
            .push(fillfrac);

        let rolling_avg = TextInput::new(
            &mut self.rolling_avg_input,
            "Rolling Average [# frames]",
            &self.rolling_avg_value,
            Message::RollingAvgChanged,
        )
        .padding(10)
        .size(20);
        let rolling_avg_label = Text::new("Rolling Average");
        let rolling_avg_row = Row::new()
            .spacing(10)
            .align_items(Align::Center)
            .push(rolling_avg_label)
            .push(rolling_avg);

        let deadtime = TextInput::new(
            &mut self.frame_dead_time_input,
            "Frame Dead Time [ms]",
            &self.frame_dead_time_value,
            Message::FrameDeadTimeChanged,
        )
        .padding(10)
        .size(20);
        let deadtime_label = Text::new("Deadtime Between Frames");
        let deadtime_row = Row::new()
            .spacing(10)
            .align_items(Align::Center)
            .push(deadtime_label)
            .push(deadtime);

        let line_shift = TextInput::new(
            &mut self.line_shift_input,
            "Line Shift [us]",
            &self.line_shift_value,
            Message::LineShiftChanged,
        )
        .padding(10)
        .size(20);
        let line_shift_label = Text::new("Line Shift [us]");
        let line_shift_row = Row::new()
            .spacing(10)
            .align_items(Align::Center)
            .push(line_shift_label)
            .push(line_shift);

        let ignored = TextInput::new(
            &mut self.ignored_channels_input,
            "Channels to ignore ('1, -4, ...')",
            &self.ignored_channels_value,
            Message::IgnoredChannelsChanged,
        )
        .padding(10)
        .size(20);
        let ignored_label = Text::new("Ignored channels");
        let ignored_row = Row::new()
            .spacing(10)
            .align_items(Align::Center)
            .push(ignored_label)
            .push(ignored);

        let pmt1 = PickList::new(
            &mut self.pmt1_pick_list,
            &ChannelNumber::ALL[..],
            Some(self.pmt1_selected),
            Message::Pmt1Changed,
        );

        let pmt1_edge = PickList::new(
            &mut self.pmt1_edge_list,
            &EdgeDetected::ALL[..],
            Some(self.pmt1_edge_selected),
            Message::Pmt1EdgeChanged,
        );

        let pmt1_thresh = TextInput::new(
            &mut self.pmt1_threshold_input,
            "PMT1 Threshold [V]",
            &self.pmt1_threshold_value,
            Message::Pmt1ThresholdChanged,
        )
        .padding(10)
        .size(20);

        let pmt1_row = Row::new()
            .spacing(10)
            .align_items(Align::Center)
            .push(Text::new("PMT 1"))
            .push(pmt1)
            .push(pmt1_edge)
            .push(pmt1_thresh);

        let pmt2 = PickList::new(
            &mut self.pmt2_pick_list,
            &ChannelNumber::ALL[..],
            Some(self.pmt2_selected),
            Message::Pmt2Changed,
        );

        let pmt2_edge = PickList::new(
            &mut self.pmt2_edge_list,
            &EdgeDetected::ALL[..],
            Some(self.pmt2_edge_selected),
            Message::Pmt2EdgeChanged,
        );

        let pmt2_thresh = TextInput::new(
            &mut self.pmt2_threshold_input,
            "PMT2 Threshold [V]",
            &self.pmt2_threshold_value,
            Message::Pmt2ThresholdChanged,
        )
        .padding(10)
        .size(20);

        let pmt2_row = Row::new()
            .spacing(10)
            .align_items(Align::Center)
            .push(Text::new("PMT 2"))
            .push(pmt2)
            .push(pmt2_edge)
            .push(pmt2_thresh);

        let pmt3 = PickList::new(
            &mut self.pmt3_pick_list,
            &ChannelNumber::ALL[..],
            Some(self.pmt3_selected),
            Message::Pmt3Changed,
        );

        let pmt3_edge = PickList::new(
            &mut self.pmt3_edge_list,
            &EdgeDetected::ALL[..],
            Some(self.pmt4_edge_selected),
            Message::Pmt3EdgeChanged,
        );

        let pmt3_thresh = TextInput::new(
            &mut self.pmt3_threshold_input,
            "PMT3 Threshold [V]",
            &self.pmt3_threshold_value,
            Message::Pmt3ThresholdChanged,
        )
        .padding(10)
        .size(20);

        let pmt3_row = Row::new()
            .spacing(10)
            .align_items(Align::Center)
            .push(Text::new("PMT 3"))
            .push(pmt3)
            .push(pmt3_edge)
            .push(pmt3_thresh);

        let pmt4 = PickList::new(
            &mut self.pmt4_pick_list,
            &ChannelNumber::ALL[..],
            Some(self.pmt4_selected),
            Message::Pmt4Changed,
        );

        let pmt4_edge = PickList::new(
            &mut self.pmt4_edge_list,
            &EdgeDetected::ALL[..],
            Some(self.pmt4_edge_selected),
            Message::Pmt4EdgeChanged,
        );

        let pmt4_thresh = TextInput::new(
            &mut self.pmt4_threshold_input,
            "PMT4 Threshold [V]",
            &self.pmt4_threshold_value,
            Message::Pmt4ThresholdChanged,
        )
        .padding(10)
        .size(20);

        let pmt4_row = Row::new()
            .spacing(10)
            .align_items(Align::Center)
            .push(Text::new("PMT 4"))
            .push(pmt4)
            .push(pmt4_edge)
            .push(pmt4_thresh);

        let laser = PickList::new(
            &mut self.laser_pick_list,
            &ChannelNumber::ALL[..],
            Some(self.laser_selected),
            Message::LaserChanged,
        );

        let laser_edge = PickList::new(
            &mut self.laser_edge_list,
            &EdgeDetected::ALL[..],
            Some(self.laser_edge_selected),
            Message::LaserEdgeChanged,
        );

        let laser_thresh = TextInput::new(
            &mut self.laser_threshold_input,
            "Laser Threshold [V]",
            &self.laser_threshold_value,
            Message::LaserThresholdChanged,
        )
        .padding(10)
        .size(20);

        let laser_row = Row::new()
            .spacing(10)
            .align_items(Align::Center)
            .push(Text::new("Laser"))
            .push(laser)
            .push(laser_edge)
            .push(laser_thresh);

        let frame = PickList::new(
            &mut self.frame_pick_list,
            &ChannelNumber::ALL[..],
            Some(self.frame_selected),
            Message::FrameChanged,
        );

        let frame_edge = PickList::new(
            &mut self.frame_edge_list,
            &EdgeDetected::ALL[..],
            Some(self.frame_edge_selected),
            Message::FrameEdgeChanged,
        );

        let frame_thresh = TextInput::new(
            &mut self.frame_threshold_input,
            "Frame Threshold [V]",
            &self.frame_threshold_value,
            Message::FrameThresholdChanged,
        )
        .padding(10)
        .size(20);

        let frame_row = Row::new()
            .spacing(10)
            .align_items(Align::Center)
            .push(Text::new("Frame"))
            .push(frame)
            .push(frame_edge)
            .push(frame_thresh);

        let line = PickList::new(
            &mut self.line_pick_list,
            &ChannelNumber::ALL[..],
            Some(self.line_selected),
            Message::LineChanged,
        );

        let line_edge = PickList::new(
            &mut self.line_edge_list,
            &EdgeDetected::ALL[..],
            Some(self.line_edge_selected),
            Message::LineEdgeChanged,
        );

        let line_thresh = TextInput::new(
            &mut self.line_threshold_input,
            "Line Threshold [V]",
            &self.line_threshold_value,
            Message::LineThresholdChanged,
        )
        .padding(10)
        .size(20);

        let line_row = Row::new()
            .spacing(10)
            .align_items(Align::Center)
            .push(Text::new("Line"))
            .push(line)
            .push(line_edge)
            .push(line_thresh);

        let taglens_input = PickList::new(
            &mut self.taglens_pick_list,
            &ChannelNumber::ALL[..],
            Some(self.taglens_selected),
            Message::TagLensChanged,
        );

        let taglens_edge = PickList::new(
            &mut self.taglens_edge_list,
            &EdgeDetected::ALL[..],
            Some(self.taglens_edge_selected),
            Message::TagLensEdgeChanged,
        );

        let taglens_thresh = TextInput::new(
            &mut self.taglens_threshold_input,
            "Tag Lens Threshold [V]",
            &self.taglens_threshold_value,
            Message::TagLensThresholdChanged,
        )
        .padding(10)
        .size(20);

        let taglens_row = Row::new()
            .spacing(10)
            .align_items(Align::Center)
            .push(Text::new("Tag Lens"))
            .push(taglens_input)
            .push(taglens_edge)
            .push(taglens_thresh);

        let bidir = Checkbox::new(
            self.bidirectional,
            "Bidirectional scan?",
            Message::BidirectionalityChanged,
        )
        .size(20);

        let run_app = Button::new(&mut self.run_button, Text::new("Start Acquistion"))
            .on_press(Message::ButtonPressed)
            .padding(10);

        let first_column = Column::new()
            .spacing(20)
            .padding(20)
            .max_width(600)
            .push(filename_row)
            .push(rows_row)
            .push(columns_row)
            .push(planes_row)
            .push(scan_period_row)
            .push(taglens_period_row)
            .push(fillfrac_row)
            .push(deadtime_row)
            .push(line_shift_row)
            .push(bidir);

        let second_column = Column::new()
            .spacing(20)
            .padding(20)
            .max_width(600)
            .push(pmt1_row)
            .push(pmt2_row)
            .push(pmt3_row)
            .push(pmt4_row)
            .push(laser_row)
            .push(frame_row)
            .push(line_row)
            .push(taglens_row)
            .push(ignored_row)
            .push(rolling_avg_row);

        let content = Column::new()
            .spacing(20)
            .padding(20)
            .max_width(1200)
            .max_height(1200)
            .align_items(Align::Center)
            .push(Image::new("resources/logo.png"))
            .push(Row::new().push(first_column).push(second_column))
            .push(run_app);

        Container::new(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x()
            .center_y()
            .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vec_to_string_full() {
        let v = vec![InputChannel::new(1, 0.0), InputChannel::new(-2, 0.0)];
        let f = vec_to_comma_sep_string(&v);
        assert_eq!(f, "1,-2".to_string());
    }

    #[test]
    fn vec_to_string_empty() {
        let v = vec![];
        let f = vec_to_comma_sep_string(&v);
        assert_eq!(f, "".to_string());
    }
}
