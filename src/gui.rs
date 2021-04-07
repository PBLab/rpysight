use std::fs::write;

use crate::{channel_value_to_pair, setup_rpysight};
use crate::{
    get_config_path,
    rendering_helpers::{AppConfig, Picosecond},
};
use iced::{
    button, pick_list, text_input, Application, Button, Checkbox, Clipboard, Column, Command,
    Container, Element, Length, PickList, Row, Text, TextInput, Image,
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
    fill_fraction_input: text_input::State,
    fill_fraction_value: String,
    frame_dead_time_input: text_input::State,
    frame_dead_time_value: String,
    pmt1_pick_list: pick_list::State<ChannelNumber>,
    pmt1_selected: ChannelNumber,
    pmt1_edge_list: pick_list::State<EdgeDetected>,
    pmt1_edge_selected: EdgeDetected,
    pmt2_pick_list: pick_list::State<ChannelNumber>,
    pmt2_selected: ChannelNumber,
    pmt2_edge_list: pick_list::State<EdgeDetected>,
    pmt2_edge_selected: EdgeDetected,
    pmt3_pick_list: pick_list::State<ChannelNumber>,
    pmt3_selected: ChannelNumber,
    pmt3_edge_list: pick_list::State<EdgeDetected>,
    pmt3_edge_selected: EdgeDetected,
    pmt4_pick_list: pick_list::State<ChannelNumber>,
    pmt4_selected: ChannelNumber,
    pmt4_edge_list: pick_list::State<EdgeDetected>,
    pmt4_edge_selected: EdgeDetected,
    laser_pick_list: pick_list::State<ChannelNumber>,
    laser_selected: ChannelNumber,
    laser_edge_list: pick_list::State<EdgeDetected>,
    laser_edge_selected: EdgeDetected,
    frame_pick_list: pick_list::State<ChannelNumber>,
    frame_selected: ChannelNumber,
    frame_edge_list: pick_list::State<EdgeDetected>,
    frame_edge_selected: EdgeDetected,
    line_pick_list: pick_list::State<ChannelNumber>,
    line_selected: ChannelNumber,
    line_edge_list: pick_list::State<EdgeDetected>,
    line_edge_selected: EdgeDetected,
    taglens_pick_list: pick_list::State<ChannelNumber>,
    taglens_selected: ChannelNumber,
    taglens_edge_list: pick_list::State<EdgeDetected>,
    taglens_edge_selected: EdgeDetected,
    run_button: button::State,
}

/// Initializes things on the Python side and starts the acquisition.
///
/// This method is called once the user clicks the "Run Application" button.
async fn start_acquisition(cfg: AppConfig) {
    let _ = save_cfg(&cfg).ok(); // Errors are logged and quite irrelevant
    info!("befire setup rpysight");
    let (window, mut app) = setup_rpysight(&cfg);
    info!("before timetagger acq started");
    app.start_timetagger_acq().expect("Failed to start TimeTagger, aborting");
    info!("after tt acq started");
    app.acquire_stream_filehandle().expect("Failed to acquire stream handle");
    info!("after acquired fh");
    window.render_loop(app);
    info!("finished rendering");
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
fn save_cfg(app_config: &AppConfig) -> anyhow::Result<()> {
    let config_path = get_config_path();
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
    Ok(())
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

    pub(crate) fn get_pmt1_channel(&self) -> (ChannelNumber, EdgeDetected) {
        (self.pmt1_selected, self.pmt1_edge_selected)
    }

    pub(crate) fn get_pmt2_channel(&self) -> (ChannelNumber, EdgeDetected) {
        (self.pmt2_selected, self.pmt2_edge_selected)
    }

    pub(crate) fn get_pmt3_channel(&self) -> (ChannelNumber, EdgeDetected) {
        (self.pmt3_selected, self.pmt3_edge_selected)
    }

    pub(crate) fn get_pmt4_channel(&self) -> (ChannelNumber, EdgeDetected) {
        (self.pmt4_selected, self.pmt4_edge_selected)
    }

    pub(crate) fn get_laser_channel(&self) -> (ChannelNumber, EdgeDetected) {
        (self.laser_selected, self.laser_edge_selected)
    }

    pub(crate) fn get_frame_channel(&self) -> (ChannelNumber, EdgeDetected) {
        (self.frame_selected, self.frame_edge_selected)
    }

    pub(crate) fn get_line_channel(&self) -> (ChannelNumber, EdgeDetected) {
        (self.line_selected, self.line_edge_selected)
    }

    pub(crate) fn get_tag_channel(&self) -> (ChannelNumber, EdgeDetected) {
        (self.taglens_selected, self.taglens_edge_selected)
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
    Pmt2Changed(ChannelNumber),
    Pmt2EdgeChanged(EdgeDetected),
    Pmt3Changed(ChannelNumber),
    Pmt3EdgeChanged(EdgeDetected),
    Pmt4Changed(ChannelNumber),
    Pmt4EdgeChanged(EdgeDetected),
    LaserChanged(ChannelNumber),
    LaserEdgeChanged(EdgeDetected),
    FrameChanged(ChannelNumber),
    FrameEdgeChanged(EdgeDetected),
    LineChanged(ChannelNumber),
    LineEdgeChanged(EdgeDetected),
    TagLensChanged(ChannelNumber),
    TagLensEdgeChanged(EdgeDetected),
    ButtonPressed,
    StartedAcquistion(()),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    Disconnected,
}

impl std::fmt::Display for ChannelNumber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                ChannelNumber::Disconnected => "Disconnected",
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
    const ALL: [ChannelNumber; 19] = [
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
        let mut app = MainAppGui::default();
        app.filename_value = prev_config.filename;
        app.rows_value = prev_config.rows.to_string();
        app.columns_value = prev_config.columns.to_string();
        app.planes_value = prev_config.planes.to_string();
        app.scan_period_value = prev_config.scan_period.to_hz().to_string();
        app.tag_period_value = prev_config.tag_period.to_hz().to_string();
        app.bidirectional = prev_config.bidir.into();
        app.fill_fraction_value = prev_config.fill_fraction.to_string();
        app.frame_dead_time_value = ps_to_ms(prev_config.frame_dead_time).to_string();
        let pmt1 = channel_value_to_pair(prev_config.pmt1_ch);
        app.pmt1_selected = pmt1.0;
        app.pmt1_edge_selected = pmt1.1;
        let pmt2 = channel_value_to_pair(prev_config.pmt2_ch);
        app.pmt2_selected = pmt2.0;
        app.pmt2_edge_selected = pmt2.1;
        let pmt3 = channel_value_to_pair(prev_config.pmt3_ch);
        app.pmt3_selected = pmt3.0;
        app.pmt3_edge_selected = pmt3.1;
        let pmt4 = channel_value_to_pair(prev_config.pmt4_ch);
        app.pmt4_selected = pmt4.0;
        app.pmt4_edge_selected = pmt4.1;
        let laser = channel_value_to_pair(prev_config.laser_ch);
        app.laser_selected = laser.0;
        app.laser_edge_selected = laser.1;
        let frame = channel_value_to_pair(prev_config.frame_ch);
        app.frame_selected = frame.0;
        app.frame_edge_selected = frame.1;
        let line = channel_value_to_pair(prev_config.line_ch);
        app.line_selected = line.0;
        app.line_edge_selected = line.1;
        let taglens = channel_value_to_pair(prev_config.taglens_ch);
        app.taglens_selected = taglens.0;
        app.taglens_edge_selected = taglens.1;

        (app, Command::none())
    }

    fn title(&self) -> String {
        String::from("RPySight 0.1.0")
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
            Message::Pmt2Changed(pmt2) => {
                self.pmt2_selected = pmt2;
                Command::none()
            }
            Message::Pmt2EdgeChanged(pmt2_edge) => {
                self.pmt2_edge_selected = pmt2_edge;
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
            Message::Pmt4Changed(pmt4) => {
                self.pmt4_selected = pmt4;
                Command::none()
            }
            Message::Pmt4EdgeChanged(pmt4_edge) => {
                self.pmt4_edge_selected = pmt4_edge;
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
            Message::FrameChanged(frame) => {
                self.frame_selected = frame;
                Command::none()
            }
            Message::FrameEdgeChanged(frame_edge) => {
                self.frame_edge_selected = frame_edge;
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
            Message::TagLensChanged(taglens) => {
                self.taglens_selected = taglens;
                Command::none()
            }
            Message::TagLensEdgeChanged(taglens_edge) => {
                self.taglens_edge_selected = taglens_edge;
                Command::none()
            }
            Message::ButtonPressed => Command::perform(start_acquisition(AppConfig::from_user_input(self).expect("")), Message::StartedAcquistion),
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
        let filename_label = Text::new("Filename").vertical_alignment(iced::VerticalAlignment::Bottom);
        let filename_row = Row::new().push(filename_label).push(filename);

        let rows = TextInput::new(
            &mut self.rows_input,
            "Rows [px]",
            &self.rows_value,
            Message::RowsChanged,
        )
        .padding(10)
        .size(20);
        let rows_label = Text::new("Rows");
        let rows_row = Row::new().push(rows_label).push(rows);

        let columns = TextInput::new(
            &mut self.columns_input,
            "Columns [px]",
            &self.columns_value,
            Message::ColumnsChanged,
        )
        .padding(10)
        .size(20);
        let columns_label = Text::new("Columns");
        let columns_row = Row::new().push(columns_label).push(columns);

        let planes = TextInput::new(
            &mut self.planes_input,
            "Planes [px] (1 for planar imaging)",
            &self.planes_value,
            Message::PlanesChanged,
        )
        .padding(10)
        .size(20);
        let planes_label = Text::new("Planes");
        let planes_row = Row::new().push(planes_label).push(planes);

        let scan_period = TextInput::new(
            &mut self.scan_period_input,
            "Scan Frequency [Hz]",
            &self.scan_period_value,
            Message::ScanPeriodChanged,
        )
        .padding(10)
        .size(20);
        let scan_period_label = Text::new("Scan Frequency");
        let scan_period_row = Row::new().push(scan_period_label).push(scan_period);

        let taglens_period = TextInput::new(
            &mut self.tag_period_input,
            "TAG Lens Frequency [Hz]",
            &self.tag_period_value,
            Message::TagLensPeriodChanged,
        )
        .padding(10)
        .size(20);
        let taglens_period_label = Text::new("TAG Lens Frequency");
        let taglens_period_row = Row::new().push(taglens_period_label).push(taglens_period);

        let fillfrac = TextInput::new(
            &mut self.fill_fraction_input,
            "Fill Fraction [%]",
            &self.fill_fraction_value,
            Message::FillFractionChanged,
        )
        .padding(10)
        .size(20);
        let fillfrac_label = Text::new("Fill Fraction");
        let fillfrac_row = Row::new().push(fillfrac_label).push(fillfrac);

        let deadtime = TextInput::new(
            &mut self.frame_dead_time_input,
            "Frame Dead Time [ms]",
            &self.frame_dead_time_value,
            Message::FrameDeadTimeChanged,
        )
        .padding(10)
        .size(20);
        let deadtime_label = Text::new("Deadtime Between Frames");
        let deadtime_row = Row::new().push(deadtime_label).push(deadtime);

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

        let bidir = Checkbox::new(
            self.bidirectional,
            "Bidirectional scan?",
            Message::BidirectionalityChanged,
        )
        .size(20);

        let run_app = Button::new(&mut self.run_button, Text::new("Start Acquistion"))
            .on_press(Message::ButtonPressed)
            .padding(10);

        let content = Column::new()
            .spacing(20)
            .padding(20)
            .max_width(600)
            .push(Image::new("resources/logo.png"))
            .push(filename_row)
            .push(rows_row)
            .push(columns_row)
            .push(planes_row)
            .push(scan_period_row)
            .push(taglens_period_row)
            .push(fillfrac_row)
            .push(deadtime_row)
            .push(bidir)
            .push(
                Row::new()
                    .push(Text::new("PMT 1"))
                    .push(pmt1)
                    .push(pmt1_edge),
            )
            .push(
                Row::new()
                    .push(Text::new("PMT 2"))
                    .push(pmt2)
                    .push(pmt2_edge),
            )
            .push(
                Row::new()
                    .push(Text::new("PMT 3"))
                    .push(pmt3)
                    .push(pmt3_edge),
            )
            .push(
                Row::new()
                    .push(Text::new("PMT 4"))
                    .push(pmt4)
                    .push(pmt4_edge),
            )
            .push(
                Row::new()
                    .push(Text::new("Laser Trigger"))
                    .push(laser)
                    .push(laser_edge),
            )
            .push(
                Row::new()
                    .push(Text::new("Frame Trigger"))
                    .push(frame)
                    .push(frame_edge),
            )
            .push(
                Row::new()
                    .push(Text::new("Line Trigger"))
                    .push(line)
                    .push(line_edge),
            )
            .push(
                Row::new()
                    .push(Text::new("TAG Lens Trigger"))
                    .push(taglens_input)
                    .push(taglens_edge),
            )
            .push(run_app);

        Container::new(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x()
            .center_y()
            .into()
    }
}
