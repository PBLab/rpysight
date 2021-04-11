#[macro_use] extern crate log;
use std::fs::File;
use std::sync::Arc;

use anyhow::{Context, Result};
use arrow::csv::Reader;
use arrow::datatypes::{DataType, Field, Schema};
use arrow::ipc::{reader::StreamReader, writer::StreamWriter};
use rand::prelude::*;
use nalgebra::Point3;
use simplelog::*;
use log::*;

use librpysight::configuration::{AppConfig, AppConfigBuilder, Inputs};
use librpysight::point_cloud_renderer::{EventStream, TimeTaggerIpcHandler, Event, ImageCoor};
use librpysight::rendering_helpers::TimeToCoord;

const GLOBAL_OFFSET: i64 = 0;
const FULL_BATCH_DATA: &'static str = "tests/data/real_record_batch.csv";
const SHORT_BATCH_DATA: &'static str = "tests/data/short_record_batch.csv";
const FULL_BATCH_STREAM: &'static str = "tests/data/real_record_batch_full_stream.dat";
const SHORT_BATCH_STREAM: &'static str = "tests/data/real_record_batch_short_stream.dat";


/// Run once to generate .dat file which behave as streams
fn test_file_to_stream() {
    let full_batch = FULL_BATCH_DATA;
    let short_batch = SHORT_BATCH_DATA;
    let schema = Schema::new(vec![
        Field::new("type_", DataType::UInt8, false),
        Field::new("missed_events", DataType::UInt16, false),
        Field::new("channel", DataType::Int32, false),
        Field::new("time", DataType::Int64, false),
    ]);
    let data_as_stream_full = File::create(FULL_BATCH_STREAM).unwrap();
    let data_as_stream_short =
        File::create(SHORT_BATCH_STREAM).unwrap();
    let mut stream_writer_full = StreamWriter::try_new(data_as_stream_full, &schema).unwrap();
    let mut stream_writer_short = StreamWriter::try_new(data_as_stream_short, &schema).unwrap();
    let mut r_full = Reader::new(
        File::open(full_batch).unwrap(),
        Arc::new(schema.clone()),
        true,
        None,
        1024,
        None,
        None,
    );
    let mut r_short = Reader::new(
        File::open(short_batch).unwrap(),
        Arc::new(schema.clone()),
        true,
        None,
        1024,
        None,
        None,
    );
    stream_writer_full
        .write(&r_full.next().unwrap().unwrap())
        .unwrap();
    stream_writer_short
        .write(&r_short.next().unwrap().unwrap())
        .unwrap();
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

fn mock_acquisition_loop(cfg: AppConfig, stream: &str) -> MockAppState {
    test_file_to_stream();
    let mut app = MockAppState::new(String::from(
        stream
), cfg);
    app.data_stream = Some(read_as_stream(
        stream
    ));
    app
}

struct MockAppState {
    data_stream_fh: String,
    pub data_stream: Option<StreamReader<File>>,
    point_color: Point3<f32>,
    time_to_coord: TimeToCoord,
    inputs: Inputs,
}

impl MockAppState {
    /// Generates a new app from a renderer and a receiving end of a channel
    pub fn new(
        data_stream_fh: String,
        appconfig: AppConfig,
    ) -> Self {
        MockAppState {
            data_stream_fh,
            data_stream: None,
            point_color: Point3::<f32>::new(1.0, 1.0, 1.0),
            time_to_coord: TimeToCoord::from_acq_params(&appconfig, GLOBAL_OFFSET),
            inputs: Inputs::from_config(&appconfig),
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
                if idx > 10 {
                    break;
                }
                if let Some(point) = self.event_to_coordinate(event) {
                    info!("This point is about to be rendered: {:?}", point);
                }
                idx += 1;
            }
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
    fn event_to_coordinate(&mut self, event: Event) -> Option<ImageCoor> {
        if event.type_ != 0 {
            return None;
        }
        info!("Received the following event: {:?}", event);
        match self.inputs[event.channel] {
            librpysight::configuration::DataType::Pmt1 => self.time_to_coord.tag_to_coord_linear(event.time),
            librpysight::configuration::DataType::Pmt2 => self.time_to_coord.dump(event.time),
            librpysight::configuration::DataType::Line => self.time_to_coord.new_line(event.time),
            librpysight::configuration::DataType::TagLens => self.time_to_coord.new_taglens_period(event.time),
            librpysight::configuration::DataType::Laser => self.time_to_coord.new_laser_event(event.time),
            _ => {
                error!("Unsupported event: {:?}", event);
                None
            }
        }
    }
}

fn setup(stream: &str) -> MockAppState {
    let _ = TestLogger::init(
        LevelFilter::Info,
        ConfigBuilder::default().set_time_to_local(true).build(),
    );

    let cfg = AppConfigBuilder::default().with_planes(1).build();
    let app = mock_acquisition_loop(cfg, stream);
    app
}

#[test]
fn assert_full_stream_exists() {
    let mut app = setup(FULL_BATCH_STREAM);
    if let Some(batch) = app.data_stream.as_mut().unwrap().next() {
        let _ = batch.unwrap();
        assert!(true)
    }
}

#[test]
fn assert_short_stream_exists() {
    let mut app = setup(SHORT_BATCH_STREAM);
    if let Some(batch) = app.data_stream.as_mut().unwrap().next() {
        let _ = batch.unwrap();
        assert!(true)
    }
}

#[test]
fn stepwise_short() {
    let mut app = setup(SHORT_BATCH_STREAM);
    app.step()
}