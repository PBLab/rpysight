extern crate log;
use std::fs::read_to_string;
use std::fs::File;
use std::sync::Arc;

use anyhow::{Context, Result};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::ipc::{reader::StreamReader, writer::StreamWriter};
use arrow::{csv::Reader, record_batch::RecordBatch};
use log::*;
use nalgebra::Point3;
use ron::de::from_str;
use simplelog::*;

use librpysight::configuration::{AppConfig, AppConfigBuilder, Bidirectionality, Inputs, Period};
use librpysight::point_cloud_renderer::{Event, EventStream, ProcessedEvent, TimeTaggerIpcHandler};
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

    fn check_relevance_of_batch(&self, event_stream: &EventStream) -> bool {
        if let Some(event) = Event::from_stream_idx(&event_stream, event_stream.num_rows() - 1) {
            let time = event.time;
            if time <= self.time_to_coord.earliest_frame_time {
                info!("The last event in the batch arrived before the first in the frame");
                false
            } else {
                true
            }
        } else {
            error!("For some reason no last event exists in this stream");
            false
        }
    }

    /// Mock step function for testing.
    /// Does not render anything, just prints out stuff.
    /// This is probably not the right way to do things.
    fn step(&mut self) {
        'step: loop {
            let batch = match self.data_stream.as_mut().unwrap().next() {
                Some(batch) => batch.expect("Test data failed"),
                None => continue,
            };
            let event_stream = match self.get_event_stream(&batch) {
                Some(stream) => stream,
                None => continue,
            };
            match self.check_relevance_of_batch(&event_stream) {
                true => {}
                false => continue,
            };
            // let mut idx = 0;
            let nanp = Point3::<f32>::new(f32::NAN, f32::NAN, f32::NAN);
            info!("Inputs: {:?}", self.inputs);
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
                    }
                    ProcessedEvent::NewFrame => break 'step,
                    ProcessedEvent::NoOp => continue,
                    ProcessedEvent::Error => self
                        .invalid_events
                        .push(TimeCoordPair::new(event.time, nanp)),
                    ProcessedEvent::FirstLine(time) => {
                        error!("First line already detected");
                        continue;
                    }
                }
                // idx += 1;
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
            return ProcessedEvent::NoOp;
        }
        info!("Received the following event: {:?}", event);
        match self.inputs[event.channel] {
            librpysight::configuration::DataType::Pmt1 => {
                self.time_to_coord.tag_to_coord_linear(event.time, 0)
            }
            librpysight::configuration::DataType::Pmt2 => {
                self.time_to_coord.tag_to_coord_linear(event.time, 1)
            }
            librpysight::configuration::DataType::Line => self.time_to_coord.new_line(event.time),
            librpysight::configuration::DataType::TagLens => {
                self.time_to_coord.new_taglens_period(event.time)
            }
            librpysight::configuration::DataType::Laser => {
                self.time_to_coord.new_laser_event(event.time)
            }
            _ => {
                error!("Unsupported event: {:?}", event);
                ProcessedEvent::Error
            }
        }
    }

    fn get_event_stream<'a>(&mut self, batch: &'a RecordBatch) -> Option<EventStream<'a>> {
        info!("Received {} rows", batch.num_rows());
        let event_stream = EventStream::from_streamed_batch(batch);
        if event_stream.num_rows() == 0 {
            debug!("A batch with 0 rows was received");
            None
        } else {
            Some(event_stream)
        }
    }
}

/// From https://stackoverflow.com/questions/40767815/how-do-i-check-whether-a-vector-is-equal-to-another-vector-that-contains-nan-and/40767977#40767977
fn eq_with_nan_eq(a: f32, b: f32) -> bool {
    (a.is_nan() && b.is_nan()) || (a == b)
}

/// From https://stackoverflow.com/questions/40767815/how-do-i-check-whether-a-vector-is-equal-to-another-vector-that-contains-nan-and/40767977#40767977
fn eq_timecoordpair_with_nan_eq(a: TimeCoordPair, b: TimeCoordPair) -> bool {
    (a.coord
        .iter()
        .zip(b.coord.iter())
        .all(|(a, b)| eq_with_nan_eq(*a, *b)))
        && (a.end_time == b.end_time)
}

/// From https://stackoverflow.com/questions/40767815/how-do-i-check-whether-a-vector-is-equal-to-another-vector-that-contains-nan-and/40767977#40767977
fn timecoordpair_vec_compare(va: &[TimeCoordPair], vb: &[TimeCoordPair]) -> bool {
    (va.len() == vb.len()) &&  // zip stops at the shortest
     va.iter()
       .zip(vb)
       .all(|(a,b)| eq_timecoordpair_with_nan_eq(*a,*b))
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
    let original_invalid: Vec<TimeCoordPair> =
        from_str(&read_to_string("tests/data/short_batch_bidir_invalid.ron").unwrap()).unwrap();
    let original_valid: Vec<TimeCoordPair> =
        from_str(&read_to_string("tests/data/short_batch_bidir_valid.ron").unwrap()).unwrap();
    assert!(timecoordpair_vec_compare(
        &app.invalid_events,
        &original_invalid
    ));
    assert!(timecoordpair_vec_compare(
        &app.valid_events,
        &original_valid
    ));
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
    let original_invalid: Vec<TimeCoordPair> =
        from_str(&read_to_string("tests/data/short_batch_unidir_invalid.ron").unwrap()).unwrap();
    let original_valid: Vec<TimeCoordPair> =
        from_str(&read_to_string("tests/data/short_batch_unidir_valid.ron").unwrap()).unwrap();
    assert!(timecoordpair_vec_compare(
        &app.invalid_events,
        &original_invalid
    ));
    assert!(timecoordpair_vec_compare(
        &app.valid_events,
        &original_valid
    ));
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
    let original_invalid: Vec<TimeCoordPair> =
        from_str(&read_to_string("tests/data/short_two_frames_batch_bidir_invalid.ron").unwrap())
            .unwrap();
    let original_valid: Vec<TimeCoordPair> =
        from_str(&read_to_string("tests/data/short_two_frames_batch_bidir_valid.ron").unwrap())
            .unwrap();
    assert!(timecoordpair_vec_compare(
        &app.invalid_events,
        &original_invalid
    ));
    assert!(timecoordpair_vec_compare(
        &app.valid_events,
        &original_valid
    ));
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
    let original_invalid: Vec<TimeCoordPair> =
        from_str(&read_to_string("tests/data/short_two_frames_batch_unidir_invalid.ron").unwrap())
            .unwrap();
    let original_valid: Vec<TimeCoordPair> =
        from_str(&read_to_string("tests/data/short_two_frames_batch_unidir_valid.ron").unwrap())
            .unwrap();
    assert!(timecoordpair_vec_compare(
        &app.invalid_events,
        &original_invalid
    ));
    assert!(timecoordpair_vec_compare(
        &app.valid_events,
        &original_valid
    ));
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
    let original_invalid: Vec<TimeCoordPair> =
        from_str(&read_to_string("tests/data/short_two_frames_batch_bidir_invalid.ron").unwrap())
            .unwrap();
    let original_valid: Vec<TimeCoordPair> =
        from_str(&read_to_string("tests/data/short_two_frames_batch_bidir_valid.ron").unwrap())
            .unwrap();
    assert!(timecoordpair_vec_compare(
        &app.invalid_events,
        &original_invalid
    ));
    assert!(timecoordpair_vec_compare(
        &app.valid_events,
        &original_valid
    ));
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
    let original_invalid: Vec<TimeCoordPair> =
        from_str(&read_to_string("tests/data/short_two_frames_batch_unidir_invalid.ron").unwrap())
            .unwrap();
    let original_valid: Vec<TimeCoordPair> =
        from_str(&read_to_string("tests/data/short_two_frames_batch_unidir_valid.ron").unwrap())
            .unwrap();
    assert!(timecoordpair_vec_compare(
        &app.invalid_events,
        &original_invalid
    ));
    assert!(timecoordpair_vec_compare(
        &app.valid_events,
        &original_valid
    ));
}
