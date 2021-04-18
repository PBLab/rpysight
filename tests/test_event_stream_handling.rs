extern crate log;
use std::fs::read_to_string;
use std::fs::File;
use std::sync::Arc;

use anyhow::{Context, Result};
use arrow::csv::Reader;
use arrow::datatypes::{DataType, Field, Schema};
use arrow::ipc::{reader::StreamReader, writer::StreamWriter};
use log::*;
use rand::prelude::*;
use ron::ser::{to_string_pretty, PrettyConfig};
use simplelog::*;

use librpysight::configuration::{AppConfig, AppConfigBuilder, Bidirectionality, Inputs, Period};
use librpysight::point_cloud_renderer::{Event, EventStream, ImageCoor, TimeTaggerIpcHandler, ProcessedEvent};
use librpysight::rendering_helpers::{Picosecond, TimeCoordPair, TimeToCoord};

const FULL_BATCH_DATA: &'static str = "tests/data/real_record_batch.csv";
const SHORT_BATCH_DATA: &'static str = "tests/data/short_record_batch.csv";
const SHORT_TWO_FRAME_BATCH_DATA: &'static str = "tests/data/short_record_batch_two_frames.csv";
const FULL_BATCH_STREAM: &'static str = "tests/data/real_record_batch_full_stream.dat";
const SHORT_BATCH_STREAM: &'static str = "tests/data/real_record_batch_short_stream.dat";
const SHORT_TWO_FRAME_BATCH_STREAM: &'static str =
    "tests/data/real_record_batch_short_two_frames_stream.dat";

/// Run once to generate .dat file which behave as streams
fn test_file_to_stream() {
    let schema = Schema::new(vec![
        Field::new("type_", DataType::UInt8, false),
        Field::new("missed_events", DataType::UInt16, false),
        Field::new("channel", DataType::Int32, false),
        Field::new("time", DataType::Int64, false),
    ]);
    for (data, stream) in vec![
        FULL_BATCH_DATA,
        SHORT_BATCH_DATA,
        SHORT_TWO_FRAME_BATCH_DATA,
    ]
    .into_iter()
    .zip(
        vec![
            FULL_BATCH_STREAM,
            SHORT_BATCH_STREAM,
            SHORT_TWO_FRAME_BATCH_STREAM,
        ]
        .into_iter(),
    ) {
        let stream_file = File::create(stream).unwrap();
        let mut stream_writer = StreamWriter::try_new(stream_file, &schema).unwrap();
        let mut r = Reader::new(
            File::open(data).unwrap(),
            Arc::new(schema.clone()),
            true,
            None,
            1024,
            None,
            None,
        );
        stream_writer.write(&r.next().unwrap().unwrap()).unwrap();
    }
}

fn read_as_stream(fname: &str) -> StreamReader<File> {
    StreamReader::try_new(File::open(fname).unwrap()).unwrap()
}

// fn read_recordbatch_short() -> RecordBatch {
//     let batch = test_file_to_stream(fname);
//     batch
// }

// fn read_recordbatch_full() -> RecordBatch {
//     let mut fname = PathBuf::new();
//     fname.push("tests/data/real_record_batch.csv");
//     let batch = read_to_recordbatch(fname);
//     batch
// }

fn mock_acquisition_loop(cfg: AppConfig, stream: &str, offset: Option<Picosecond>) -> MockAppState {
    test_file_to_stream();
    let offset = offset.unwrap_or(0);
    let mut app = MockAppState::new(String::from(stream), cfg, offset);
    app.data_stream = Some(read_as_stream(stream));
    app
}

struct MockAppState {
    data_stream_fh: String,
    pub data_stream: Option<StreamReader<File>>,
    time_to_coord: TimeToCoord,
    inputs: Inputs,
    valid_events: Vec<TimeCoordPair>,
    invalid_events: Vec<TimeCoordPair>,
}

impl MockAppState {
    /// Generates a new app from a renderer and a receiving end of a channel
    pub fn new(data_stream_fh: String, appconfig: AppConfig, offset: Picosecond) -> Self {
        MockAppState {
            data_stream_fh,
            data_stream: None,
            time_to_coord: TimeToCoord::from_acq_params(&appconfig, offset),
            inputs: Inputs::from_config(&appconfig),
            valid_events: Vec::<TimeCoordPair>::with_capacity(100_000),
            invalid_events: Vec::<TimeCoordPair>::with_capacity(100_000),
        }
    }

    pub fn get_data_from_channel(&self, length: usize) -> Vec<ImageCoor> {
        let mut rng = rand::thread_rng();
        let mut data = Vec::with_capacity(10_000);
        for _ in 0..length {
            let x: f32 = rng.gen::<f32>();
            let y: f32 = rng.gen::<f32>();
            let z: f32 = rng.gen::<f32>();
            let point = ImageCoor::new(x, y, z);
            data.push(point);
        }
        data
    }

    /// Mock step function for testing.
    /// Does not render anything, just prints out stuff.
    /// This is probably not the right way to do things.
    fn step(&mut self) {
        if let Some(batch) = self.data_stream.as_mut().unwrap().next() {
            let batch = batch.unwrap();
            // info!("Received {} rows", batch.num_rows());
            // let v = self.get_data_from_channel(batch.num_rows());
            // for p in v {
            //     info!("This point is about to be rendered: {:?}", p);
            // }
            let mut idx = 0;
            let event_stream = EventStream::from_streamed_batch(&batch);
            if Event::from_stream_idx(&event_stream, event_stream.num_rows() - 1).time
                <= self.time_to_coord.earliest_frame_time
            {
                info!("The last event in the batch arrived before the first in the frame");
                return;
            } else {
                info!("Last event is later than the first");
            }
            for event in event_stream.into_iter() {
                // if idx > 10 {
                //     break;
                // }
                match self.event_to_coordinate(event) {
                    ProcessedEvent::Displayed(point, _) => {
                        info!("This point is about to be rendered: {:?}", point);
                        if point.iter().copied().any(|x| x.is_nan()) {
                            self.invalid_events
                                .push(TimeCoordPair::new(event.time, point));
                        } else {
                            self.valid_events
                                .push(TimeCoordPair::new(event.time, point));
                        }
                    },
                    ProcessedEvent::NewFrame => {continue},
                    ProcessedEvent::NoOp => { continue },
                }
                idx += 1;
            }
            // write(
            //     "tests/data/short_two_frames_with_offset_batch_unidir_valid.ron",
            //     to_string_pretty(&self.valid_events, PrettyConfig::new()).unwrap(),
            // )
            // .unwrap();
            // write(
            //     "tests/data/short_two_frames_with_offset_batch_unidir_invalid.ron",
            //     to_string_pretty(&self.invalid_events, PrettyConfig::new()).unwrap(),
            // )
            // .unwrap();
        }
    }
}

impl TimeTaggerIpcHandler for MockAppState {
    /// Instantiate an IPC StreamReader using an existing file handle.
    fn acquire_stream_filehandle(&mut self) -> Result<()> {
        let stream =
            File::open(&self.data_stream_fh).context("Can't open stream file, exiting.")?;
        let stream =
            StreamReader::try_new(stream).context("Stream file missing, cannot recover.")?;
        self.data_stream = Some(stream);
        Ok(())
    }

    /// Convert a raw event tag to a coordinate which will be displayed on the
    /// screen.
    ///
    /// This is the core of the rendering logic of this application, where all
    /// metadata (row, column info) is used to decide where to place a given
    /// event.
    ///
    /// None is returned if the tag isn't a time tag. When the tag is from a
    /// non-imaging channel it's taken into account, but otherwise (i.e. in
    /// cases of overflow it's discarded at the moment.
    fn event_to_coordinate(&mut self, event: Event) -> ProcessedEvent {
        if event.type_ != 0 {
            return ProcessedEvent::NoOp
        }
        info!("Received the following event: {:?}", event);
        match self.inputs[event.channel] {
            librpysight::configuration::DataType::Pmt1 => {
                self.time_to_coord.tag_to_coord_linear(event.time, 0)
            }
            librpysight::configuration::DataType::Pmt2 => self.time_to_coord.tag_to_coord_linear(event.time, 1),
            librpysight::configuration::DataType::Line => self.time_to_coord.new_line(event.time),
            librpysight::configuration::DataType::TagLens => {
                self.time_to_coord.new_taglens_period(event.time)
            }
            librpysight::configuration::DataType::Laser => {
                self.time_to_coord.new_laser_event(event.time)
            }
            _ => {
                error!("Unsupported event: {:?}", event);
                ProcessedEvent::NoOp
            }
        }
    }
}

/// Start a logger, generate a default config file (if given none) and generate
/// a data stream from one of the CSV files.
fn setup(csv_to_stream: &str, cfg: Option<AppConfig>, offset: Option<Picosecond>) -> MockAppState {
    let _ = TestLogger::init(
        LevelFilter::Info,
        ConfigBuilder::default().set_time_to_local(true).build(),
    );
    let cfg = cfg.unwrap_or(AppConfigBuilder::default().with_planes(1).build());
    let app = mock_acquisition_loop(cfg, csv_to_stream, offset);
    app
}

#[test]
fn assert_full_stream_exists() {
    test_file_to_stream();
    let mut app = setup(FULL_BATCH_STREAM, None, None);
    if let Some(batch) = app.data_stream.as_mut().unwrap().next() {
        let _ = batch.unwrap();
        assert!(true)
    }
}

#[test]
fn assert_short_stream_exists() {
    test_file_to_stream();
    let mut app = setup(SHORT_BATCH_STREAM, None, None);
    if let Some(batch) = app.data_stream.as_mut().unwrap().next() {
        let _ = batch.unwrap();
        assert!(true)
    }
}

#[test]
fn stepwise_short_bidir_single_frame() {
    let cfg: AppConfig = AppConfigBuilder::default()
        .with_scan_period(Period::from_freq(100_000.0))
        .with_columns(10)
        .with_rows(10)
        .with_planes(1)
        .with_bidir(Bidirectionality::Bidir)
        .build();
    let mut app = setup(SHORT_BATCH_STREAM, Some(cfg), None);
    app.step();
    assert_eq!(
        to_string_pretty(&app.invalid_events, PrettyConfig::new()).unwrap(),
        read_to_string("tests/data/short_batch_bidir_invalid.ron").unwrap()
    );
    assert_eq!(
        to_string_pretty(&app.valid_events, PrettyConfig::new()).unwrap(),
        read_to_string("tests/data/short_batch_bidir_valid.ron").unwrap()
    );
}

#[test]
fn stepwise_short_unidir_single_frame() {
    let cfg: AppConfig = AppConfigBuilder::default()
        .with_scan_period(Period::from_freq(100_000.0))
        .with_columns(10)
        .with_rows(10)
        .with_planes(1)
        .with_bidir(Bidirectionality::Unidir)
        .build();
    let mut app = setup(SHORT_BATCH_STREAM, Some(cfg), None);
    app.step();
    assert_eq!(
        to_string_pretty(&app.invalid_events, PrettyConfig::new()).unwrap(),
        read_to_string("tests/data/short_batch_unidir_invalid.ron").unwrap()
    );
    assert_eq!(
        to_string_pretty(&app.valid_events, PrettyConfig::new()).unwrap(),
        read_to_string("tests/data/short_batch_unidir_valid.ron").unwrap()
    );
}

#[test]
fn stepwise_short_two_frames_bidir() {
    let cfg: AppConfig = AppConfigBuilder::default()
        .with_scan_period(Period::from_freq(100_000.0))
        .with_columns(10)
        .with_rows(10)
        .with_planes(1)
        .with_bidir(Bidirectionality::Bidir)
        .with_frame_dead_time(10_000_000)
        .build();
    let mut app = setup(SHORT_TWO_FRAME_BATCH_STREAM, Some(cfg), None);
    app.step();
    assert_eq!(
        to_string_pretty(&app.invalid_events, PrettyConfig::new()).unwrap(),
        read_to_string("tests/data/short_two_frames_batch_bidir_invalid.ron").unwrap()
    );
    assert_eq!(
        to_string_pretty(&app.valid_events, PrettyConfig::new()).unwrap(),
        read_to_string("tests/data/short_two_frames_batch_bidir_valid.ron").unwrap()
    );
}

#[test]
fn stepwise_short_two_frames_unidir() {
    let cfg: AppConfig = AppConfigBuilder::default()
        .with_scan_period(Period::from_freq(100_000.0))
        .with_columns(10)
        .with_rows(10)
        .with_planes(1)
        .with_bidir(Bidirectionality::Unidir)
        .with_frame_dead_time(10_000_000)
        .build();
    let mut app = setup(SHORT_TWO_FRAME_BATCH_STREAM, Some(cfg), None);
    app.step();
    assert_eq!(
        to_string_pretty(&app.invalid_events, PrettyConfig::new()).unwrap(),
        read_to_string("tests/data/short_two_frames_batch_unidir_invalid.ron").unwrap()
    );
    assert_eq!(
        to_string_pretty(&app.valid_events, PrettyConfig::new()).unwrap(),
        read_to_string("tests/data/short_two_frames_batch_unidir_valid.ron").unwrap()
    );
}

#[test]
fn stepwise_short_two_frames_offset_bidir() {
    let cfg: AppConfig = AppConfigBuilder::default()
        .with_scan_period(Period::from_freq(100_000.0))
        .with_columns(10)
        .with_rows(10)
        .with_planes(1)
        .with_bidir(Bidirectionality::Bidir)
        .with_frame_dead_time(10_000_000)
        .build();
    let mut app = setup(SHORT_TWO_FRAME_BATCH_STREAM, Some(cfg), Some(100));
    app.step();
    assert_eq!(
        to_string_pretty(&app.invalid_events, PrettyConfig::new()).unwrap(),
        read_to_string("tests/data/short_two_frames_batch_bidir_invalid.ron").unwrap()
    );
    assert_eq!(
        to_string_pretty(&app.valid_events, PrettyConfig::new()).unwrap(),
        read_to_string("tests/data/short_two_frames_batch_bidir_valid.ron").unwrap()
    );
}

#[test]
fn stepwise_short_two_frames_offset_unidir() {
    let cfg: AppConfig = AppConfigBuilder::default()
        .with_scan_period(Period::from_freq(100_000.0))
        .with_columns(10)
        .with_rows(10)
        .with_planes(1)
        .with_bidir(Bidirectionality::Unidir)
        .with_frame_dead_time(10_000_000)
        .build();
    let mut app = setup(SHORT_TWO_FRAME_BATCH_STREAM, Some(cfg), Some(100));
    app.step();
    assert_eq!(
        to_string_pretty(&app.invalid_events, PrettyConfig::new()).unwrap(),
        read_to_string("tests/data/short_two_frames_batch_unidir_invalid.ron").unwrap()
    );
    assert_eq!(
        to_string_pretty(&app.valid_events, PrettyConfig::new()).unwrap(),
        read_to_string("tests/data/short_two_frames_batch_unidir_valid.ron").unwrap()
    );
}
