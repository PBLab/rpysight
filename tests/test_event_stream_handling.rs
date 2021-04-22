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
use ron::{ser::{to_writer_pretty, PrettyConfig}, de::from_reader};
use simplelog::*;
use kiss3d::window::Window;
use kiss3d::event;
use kiss3d::camera::Camera;
use kiss3d::renderer::Renderer;
use serde::{Serialize, Deserialize};

use librpysight::configuration::{AppConfig, AppConfigBuilder, Bidirectionality, Inputs, Period};
use librpysight::point_cloud_renderer::{Event, EventStream, ProcessedEvent, TimeTaggerIpcHandler, AppState, PointDisplay};
use librpysight::rendering_helpers::{Picosecond, TimeCoordPair, TimeToCoord};
use librpysight::setup_renderer;

const FULL_BATCH_DATA: &'static str = "tests/data/real_record_batch.csv";
const SHORT_BATCH_DATA: &'static str = "tests/data/short_record_batch.csv";
const SHORT_TWO_FRAME_BATCH_DATA: &'static str = "tests/data/short_record_batch_two_frames.csv";
const WITH_LINES_DATA: &'static str = "tests/data/record_batch_with_lines.csv";
const FULL_BATCH_STREAM: &'static str = "tests/data/real_record_batch_full_stream.dat";
const SHORT_BATCH_STREAM: &'static str = "tests/data/real_record_batch_short_stream.dat";
const SHORT_TWO_FRAME_BATCH_STREAM: &'static str =
    "tests/data/real_record_batch_short_two_frames_stream.dat";
const WITH_LINES_STREAM: &'static str = "tests/data/record_batch_with_lines.dat";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
struct PointLogger {
    rendered_events_loc: Vec<TimeCoordPair>,
    rendered_events_color: Vec<TimeCoordPair>,
}

impl PointLogger {
    pub fn new() -> Self {
        PointLogger { rendered_events_loc: Vec::<TimeCoordPair>::new(), rendered_events_color: Vec::<TimeCoordPair>::new() }
    }
}

impl PointDisplay for PointLogger {
    fn display_point(&mut self, p: Point3<f32>, c: Point3<f32>, time: Picosecond) {
        self.rendered_events_loc.push(TimeCoordPair::new(time, p));
        self.rendered_events_color.push(TimeCoordPair::new(time, c));
    }
}

impl Renderer for PointLogger {
    fn render(&mut self, _pass: usize, _camera: &mut dyn Camera) { }
}

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
        WITH_LINES_DATA,
    ]
    .into_iter()
    .zip(
        vec![
            FULL_BATCH_STREAM,
            SHORT_BATCH_STREAM,
            SHORT_TWO_FRAME_BATCH_STREAM,
            WITH_LINES_STREAM,
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
fn setup(csv_to_stream: &str, cfg: Option<AppConfig>) -> (Window, AppState<PointLogger, File>) {
    let _ = TestLogger::init(
        LevelFilter::Debug,
        ConfigBuilder::default().set_time_to_local(true).build(),
    );
    test_file_to_stream();
    let cfg = cfg.unwrap_or(AppConfigBuilder::default().with_planes(1).build());
    let mut window = Window::new("rPySight 0.1.0 test");
    let mut app = setup_renderer(&mut window, PointLogger::new(), &cfg, csv_to_stream.to_string());
    app.acquire_stream_filehandle().unwrap();
    window.hide();
    (window, app)
}

#[test]
fn assert_full_stream_exists() {
    test_file_to_stream();
    let (window, mut app) = setup(FULL_BATCH_STREAM, None);
    if let Some(batch) = app.data_stream.as_mut().unwrap().next() {
        let _ = batch.unwrap();
        assert!(true)
    }
}

#[test]
fn assert_short_stream_exists() {
    test_file_to_stream();
    let (window, mut app) = setup(SHORT_BATCH_STREAM, None, None);
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
    let (mut window, mut app) = setup(SHORT_BATCH_STREAM, Some(cfg), None);
    window.render_with_state(&mut app);
    let original_invalid: Vec<TimeCoordPair> =
        from_str(&read_to_string("tests/data/short_batch_bidir_invalid.ron").unwrap()).unwrap();
    let original_valid: Vec<TimeCoordPair> =
        from_str(&read_to_string("tests/data/short_batch_bidir_valid.ron").unwrap()).unwrap();
    let mut v = Vec::new();
    window.snap(&mut v);
    println!("{:?}", v);
    // assert!(timecoordpair_vec_compare(
    //     &app.invalid_events,
    //     &original_invalid
    // ));
    // assert!(timecoordpair_vec_compare(
    //     &app.valid_events,
    //     &original_valid
    // ));
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
    let (mut window, mut app) = setup(SHORT_BATCH_STREAM, Some(cfg), None);
    window.render_with_state(&mut app);
    let original_invalid: Vec<TimeCoordPair> =
        from_str(&read_to_string("tests/data/short_batch_unidir_invalid.ron").unwrap()).unwrap();
    let original_valid: Vec<TimeCoordPair> =
        from_str(&read_to_string("tests/data/short_batch_unidir_valid.ron").unwrap()).unwrap();
    // assert!(timecoordpair_vec_compare(
    //     &app.invalid_events,
    //     &original_invalid
    // ));
    // assert!(timecoordpair_vec_compare(
    //     &app.valid_events,
    //     &original_valid
    // ));
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
    let (window, mut app) = setup(SHORT_TWO_FRAME_BATCH_STREAM, Some(cfg), None);
    // app.step();
    // let original_invalid: Vec<TimeCoordPair> =
    //     from_str(&read_to_string("tests/data/short_two_frames_batch_bidir_invalid.ron").unwrap())
    //         .unwrap();
    // let original_valid: Vec<TimeCoordPair> =
    //     from_str(&read_to_string("tests/data/short_two_frames_batch_bidir_valid.ron").unwrap())
    //         .unwrap();
    // assert!(timecoordpair_vec_compare(
    //     &app.invalid_events,
    //     &original_invalid
    // ));
    // assert!(timecoordpair_vec_compare(
    //     &app.valid_events,
    //     &original_valid
    // ));
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
    let (window, mut app) = setup(SHORT_TWO_FRAME_BATCH_STREAM, Some(cfg), None);
    // app.step();
    // let original_invalid: Vec<TimeCoordPair> =
    //     from_str(&read_to_string("tests/data/short_two_frames_batch_unidir_invalid.ron").unwrap())
    //         .unwrap();
    // let original_valid: Vec<TimeCoordPair> =
    //     from_str(&read_to_string("tests/data/short_two_frames_batch_unidir_valid.ron").unwrap())
    //         .unwrap();
    // assert!(timecoordpair_vec_compare(
    //     &app.invalid_events,
    //     &original_invalid
    // ));
    // assert!(timecoordpair_vec_compare(
    //     &app.valid_events,
    //     &original_valid
    // ));
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
    let (window, mut app) = setup(SHORT_TWO_FRAME_BATCH_STREAM, Some(cfg), Some(100));
    // app.step();
    // let original_invalid: Vec<TimeCoordPair> =
    //     from_str(&read_to_string("tests/data/short_two_frames_batch_bidir_invalid.ron").unwrap())
    //         .unwrap();
    // let original_valid: Vec<TimeCoordPair> =
    //     from_str(&read_to_string("tests/data/short_two_frames_batch_bidir_valid.ron").unwrap())
    //         .unwrap();
    // assert!(timecoordpair_vec_compare(
    //     &app.invalid_events,
    //     &original_invalid
    // ));
    // assert!(timecoordpair_vec_compare(
    //     &app.valid_events,
    //     &original_valid
    // ));
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
        .with_line_ch(9)
        .build();
    let (mut window, mut app) = setup(SHORT_TWO_FRAME_BATCH_STREAM, Some(cfg), Some(100));
    window.render_with_state(&mut app);
    // window.close();
    // // app.step();
    // let original_invalid: Vec<TimeCoordPair> =
    //     from_str(&read_to_string("tests/data/short_two_frames_batch_unidir_invalid.ron").unwrap())
    //         .unwrap();
    // let original_valid: Vec<TimeCoordPair> =
    //     from_str(&read_to_string("tests/data/short_two_frames_batch_unidir_valid.ron").unwrap())
    //         .unwrap();
    // assert!(timecoordpair_vec_compare(
    //     &app.invalid_events,
    //     &original_invalid
    // ));
    // assert!(timecoordpair_vec_compare(
    //     &app.valid_events,
    //     &original_valid
    // ));
}

#[test]
fn offset_with_lines() {
    let cfg: AppConfig = AppConfigBuilder::default()
        .with_planes(1)
        .with_rows(2)
        .with_columns(2)
        .with_pmt1_ch(-3)
        .with_pmt2_ch(-8)
        .with_line_ch(1)
        .build();
    let (mut window, mut app) = setup(WITH_LINES_STREAM, Some(cfg));
    window.render_with_state(&mut app);
    to_writer_pretty(File::create("tests/data/record_batch_with_lines.ron").unwrap(), &app.renderer, PrettyConfig::new()).unwrap();
    let original: PointLogger = from_reader(File::open("tests/data/record_batch_with_lines_loc.ron").unwrap()).unwrap();  
    assert_eq!(original, app.renderer);
}
