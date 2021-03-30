use iced::{
    button, scrollable, slider, text_input, Align, Button, Checkbox, Column,
    Container, Element, Length, ProgressBar, Radio, Row, Rule, Sandbox,
    Scrollable, Settings, Slider, Space, Text, TextInput, pick_list, PickList,
};

use crate::rendering_helpers::AppConfig;


pub fn run_appconfig_gui() -> iced::Result {
    ConfigGui::run(Settings::default())
}

#[derive(Default)]
struct ConfigGui {
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

#[derive(Debug, Clone)]
enum Message {
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
    // ButtonPressed,
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChannelNumber {
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
    Empty,
}

impl std::fmt::Display for ChannelNumber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                ChannelNumber::Empty => "Empty",
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
        ChannelNumber::Empty,
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
    fn default() -> Self { ChannelNumber::Empty }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EdgeDetected {
    Rising,
    Falling,
}

impl EdgeDetected {
    const ALL: [EdgeDetected; 2] = [EdgeDetected::Rising, EdgeDetected::Falling];
}

impl Default for EdgeDetected {
    fn default() -> Self { EdgeDetected::Rising }
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

impl Sandbox for ConfigGui {
    type Message = Message;

    fn new() -> Self {
        ConfigGui::default()
    }

    fn title(&self) -> String {
        String::from("RPySight 0.1.0")
    }

    fn update(&mut self, message: Message) {
        match message {
            Message::RowsChanged(rows) => self.rows_value = rows,
            Message::ColumnsChanged(columns) => self.columns_value = columns,
            Message::PlanesChanged(planes) => self.planes_value = planes,
            Message::ScanPeriodChanged(period) => self.scan_period_value = period,
            Message::TagLensPeriodChanged(period) => self.tag_period_value = period,
            Message::BidirectionalityChanged(bidir) => self.bidirectional = bidir,
            Message::FillFractionChanged(fillfrac) => self.fill_fraction_value = fillfrac,
            Message::FrameDeadTimeChanged(deadtime) => self.frame_dead_time_value = deadtime,
            Message::Pmt1Changed(pmt1) => self.pmt1_selected = pmt1,
            Message::Pmt1EdgeChanged(pmt1_edge) => self.pmt1_edge_selected = pmt1_edge,
            Message::Pmt2Changed(pmt2) => self.pmt2_selected = pmt2,
            Message::Pmt2EdgeChanged(pmt2_edge) => self.pmt2_edge_selected = pmt2_edge,
            Message::Pmt3Changed(pmt3) => self.pmt3_selected = pmt3,
            Message::Pmt3EdgeChanged(pmt3_edge) => self.pmt3_edge_selected = pmt3_edge,
            Message::Pmt4Changed(pmt4) => self.pmt4_selected = pmt4,
            Message::Pmt4EdgeChanged(pmt4_edge) => self.pmt4_edge_selected = pmt4_edge,
            Message::LaserChanged(laser) => self.laser_selected = laser,
            Message::LaserEdgeChanged(laser_edge) => self.laser_edge_selected = laser_edge,
            Message::FrameChanged(frame) => self.frame_selected = frame,
            Message::FrameEdgeChanged(frame_edge) => self.frame_edge_selected = frame_edge,
            Message::LineChanged(line) => self.line_selected = line,
            Message::LineEdgeChanged(line_edge) => self.line_edge_selected = line_edge,
            Message::TagLensChanged(taglens) => self.taglens_selected = taglens,
            Message::TagLensEdgeChanged(taglens_edge) => self.taglens_edge_selected = taglens_edge,
            // Message::ButtonPressed => self.return(),
        }
    }

    fn view(&mut self) -> Element<Message> {
        let rows = TextInput::new(
            &mut self.rows_input,
            "Rows [px]",
            &self.rows_value,
            Message::RowsChanged,
        )
        .padding(10)
        .size(20);

        let columns = TextInput::new(
            &mut self.columns_input,
            "Columns [px]",
            &self.columns_value,
            Message::ColumnsChanged,
        )
        .padding(10)
        .size(20);

        let planes = TextInput::new(
            &mut self.planes_input,
            "Planes [px] (1 for planar imaging)",
            &self.planes_value,
            Message::PlanesChanged,
        )
        .padding(10)
        .size(20);

        let scan_period = TextInput::new(
            &mut self.scan_period_input,
            "Scan Period [Hz]",
            &self.scan_period_value,
            Message::ScanPeriodChanged,
        )
        .padding(10)
        .size(20);

        let taglens_period = TextInput::new(
            &mut self.tag_period_input,
            "TAG Lens Period [Hz]",
            &self.tag_period_value,
            Message::TagLensPeriodChanged,
        )
        .padding(10)
        .size(20);

        let fillfrac = TextInput::new(
            &mut self.fill_fraction_input,
            "Fill Fraction [%]",
            &self.fill_fraction_value,
            Message::FillFractionChanged,
        )
        .padding(10)
        .size(20);

        let deadtime = TextInput::new(
            &mut self.frame_dead_time_input,
            "Frame Dead Time [ms]",
            &self.frame_dead_time_value,
            Message::FrameDeadTimeChanged,
        )
        .padding(10)
        .size(20);

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

        let content = Column::new()
            .spacing(20)
            .padding(20)
            .max_width(600)
            .push(rows)
            .push(columns)
            .push(planes)
            .push(scan_period)
            .push(taglens_period)
            .push(fillfrac)
            .push(deadtime)
            .push(bidir)
            .push(Row::new().push(Text::new("PMT 1")).push(pmt1).push(pmt1_edge))
            .push(Row::new().push(Text::new("PMT 2")).push(pmt2).push(pmt2_edge))
            .push(Row::new().push(Text::new("PMT 3")).push(pmt3).push(pmt3_edge))
            .push(Row::new().push(Text::new("PMT 4")).push(pmt4).push(pmt4_edge))
            .push(Row::new().push(Text::new("Laser Trigger")).push(laser).push(laser_edge))
            .push(Row::new().push(Text::new("Frame Trigger")).push(frame).push(frame_edge))
            .push(Row::new().push(Text::new("Line Trigger")).push(line).push(line_edge))
            .push(Row::new().push(Text::new("TAG Lens Trigger")).push(taglens_input).push(taglens_edge));

        Container::new(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x()
            .center_y()
            .into()
    }
}
