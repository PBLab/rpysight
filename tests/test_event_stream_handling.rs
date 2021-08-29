extern crate log;
use std::fs::File;
use std::sync::Arc;

use arrow2::datatypes::{DataType, Field, Schema};
use arrow2::io::csv::read::{Reader, ReaderBuilder};
use arrow2::io::ipc::write::StreamWriter;
use log::*;
use nalgebra::Point3;
use ordered_float::OrderedFloat;
use ron::de::from_reader;
use serde::{Deserialize, Serialize};

use librpysight::configuration::{
    AppConfig, AppConfigBuilder, Bidirectionality, InputChannel, Period,
};
use librpysight::point_cloud_renderer::{
    AppState, ChannelNames, Channels, ImageCoor, PointDisplay, TimeTaggerIpcHandler,
};
use librpysight::snakes::{Picosecond, TimeCoordPair};

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
    fn new() -> Self {
        PointLogger {
            rendered_events_loc: Vec::<TimeCoordPair>::new(),
            rendered_events_color: Vec::<TimeCoordPair>::new(),
        }
    }
}

impl PointDisplay for PointLogger {
    fn display_point(&mut self, p: &ImageCoor, c: &Point3<f32>, time: Picosecond) {
        let contains_nan = p.iter().any(|x| x.is_nan());
        if contains_nan {
            return;
        };
        self.rendered_events_loc.push(TimeCoordPair::new(time, *p));
        self.rendered_events_color.push(TimeCoordPair::new(
            time,
            Point3::new(c.x.into(), c.y.into(), c.z.into()),
        ));
    }

    fn render(&mut self) {}
    fn hide(&mut self) {}
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
        let mut reader = ReaderBuilder::new().from_path(data).unwrap();
        info!("Reader initialized, writing data");
        stream_writer
            .write(&reader.records().next().unwrap().unwrap())
            .unwrap();
    }
}

fn generate_mock_channels() -> Channels<PointLogger> {
    let mut plvec = Vec::new();
    for _ in 0..5 {
        plvec.push(PointLogger::new());
    }
    Channels::new(plvec)
}

pub fn setup_logger() {
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
        .chain(std::io::stdout())
        .apply()
        .unwrap();
}

/// Start a logger, generate a default config file (if given none) and generate
/// a data stream from one of the CSV files.
fn setup(csv_to_stream: &str, cfg: Option<AppConfig>) -> AppState<PointLogger, StreamReader<File>> {
    setup_logger();
    test_file_to_stream();
    let cfg = cfg.unwrap_or(AppConfigBuilder::default().with_planes(1).build());
    let channels = generate_mock_channels();
    info!("{:?}", channels);
    let mut app = AppState::new(channels, csv_to_stream.to_string(), cfg);
    app.acquire_stream_filehandle().unwrap();
    app.channels.hide_all();
    app
}

#[test]
fn assert_full_stream_exists() {
    test_file_to_stream();
    let mut app = setup(FULL_BATCH_STREAM, None);
    if let Some(batch) = app.stream.get_mut_data_stream().unwrap().next() {
        let _ = batch.unwrap();
        assert!(true)
    }
}

#[test]
fn assert_short_stream_exists() {
    test_file_to_stream();
    let mut app = setup(SHORT_BATCH_STREAM, None);
    if let Some(batch) = app.stream.get_mut_data_stream().unwrap().next() {
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
        .with_line_ch(InputChannel::new(9, 0.0))
        .build();
    let mut app = setup(SHORT_BATCH_STREAM, Some(cfg));
    app.start_acq_loop_for(1).unwrap();
    // to_writer_pretty(File::create("tests/data/short_batch_bidir_valid.ron").unwrap(), &app.channels[ChannelNames::ChannelMerge], PrettyConfig::new()).unwrap();
    let original: PointLogger =
        from_reader(File::open("tests/data/short_batch_bidir_valid.ron").unwrap()).unwrap();
    assert_eq!(app.channels[ChannelNames::ChannelMerge], original);
}

#[test]
fn stepwise_short_unidir_single_frame() {
    let cfg: AppConfig = AppConfigBuilder::default()
        .with_scan_period(Period::from_freq(100_000.0))
        .with_columns(10)
        .with_rows(10)
        .with_planes(1)
        .with_line_ch(InputChannel::new(9, 0.0))
        .with_bidir(Bidirectionality::Unidir)
        .build();
    let mut app = setup(SHORT_BATCH_STREAM, Some(cfg));
    app.start_acq_loop_for(1).unwrap();
    // to_writer_pretty(File::create("tests/data/short_batch_unidir_valid.ron").unwrap(), &app.channels[ChannelNames::ChannelMerge], PrettyConfig::new()).unwrap();
    let original: PointLogger =
        from_reader(File::open("tests/data/short_batch_unidir_valid.ron").unwrap()).unwrap();
    assert_eq!(app.channels[ChannelNames::ChannelMerge], original);
}

// #[test]
// fn stepwise_short_two_frames_bidir() {
//     let cfg: AppConfig = AppConfigBuilder::default()
//         .with_scan_period(Period::from_freq(100_000.0))
//         .with_columns(10)
//         .with_rows(10)
//         .with_planes(1)
//         .with_line_ch(InputChannel::new(9, 0.0))
//         .with_bidir(Bidirectionality::Bidir)
//         .with_frame_dead_time(10_000_000)
//         .build();
//     let mut app = setup(SHORT_TWO_FRAME_BATCH_STREAM, Some(cfg));
//     app.start_acq_loop_for(2).unwrap();
//     let fname = "tests/data/short_two_frames_batch_bidir_valid.ron";
//     // to_writer_pretty(File::create(fname).unwrap(), &app.channels[ChannelNames::ChannelMerge], PrettyConfig::new()).unwrap();
//     let original: PointLogger =
//         from_reader(File::open(fname).unwrap()).unwrap();
//     assert_eq!(app.channels[ChannelNames::ChannelMerge], original);
// }

// #[test]
// fn stepwise_short_two_frames_unidir() {
//     let cfg: AppConfig = AppConfigBuilder::default()
//         .with_scan_period(Period::from_freq(100_000.0))
//         .with_columns(10)
//         .with_rows(10)
//         .with_planes(1)
//         .with_bidir(Bidirectionality::Unidir)
//         .with_frame_dead_time(10_000_000)
//         .with_line_ch(InputChannel::new(9, 0.0))
//         .build();
//     let mut app = setup(SHORT_TWO_FRAME_BATCH_STREAM, Some(cfg));
//     app.start_acq_loop_for(3).unwrap();
//     // to_writer_pretty(File::create("tests/data/short_two_frames_batch_unidir_valid.ron").unwrap(), &app.channels[ChannelNames::ChannelMerge], PrettyConfig::new()).unwrap();
//     let original: PointLogger =
//         from_reader(File::open("tests/data/short_two_frames_batch_unidir_valid.ron").unwrap())
//             .unwrap();
//     assert_eq!(app.channels[ChannelNames::ChannelMerge], original)
// }

// // #[test]
// // fn stepwise_short_two_frames_offset_bidir() {
// //     let cfg: AppConfig = AppConfigBuilder::default()
// //         .with_scan_period(Period::from_freq(100_000.0))
// //         .with_columns(10)
// //         .with_rows(10)
// //         .with_planes(1)
// //         .with_bidir(Bidirectionality::Bidir)
// //         .with_frame_dead_time(10_000_000)
// //         .build();
// //     let (window, mut app) = setup(SHORT_TWO_FRAME_BATCH_STREAM, Some(cfg), Some(100));
//     // app.step();
//     // let original_invalid: Vec<TimeCoordPair> =
//     //     from_str(&read_to_string("tests/data/short_two_frames_batch_bidir_invalid.ron").unwrap())
//     //         .unwrap();
//     // let original_valid: Vec<TimeCoordPair> =
//     //     from_str(&read_to_string("tests/data/short_two_frames_batch_bidir_valid.ron").unwrap())
//     //         .unwrap();
//     // assert!(timecoordpair_vec_compare(
//     //     &app.invalid_events,
//     //     &original_invalid
//     // ));
//     // assert!(timecoordpair_vec_compare(
//     //     &app.valid_events,
//     //     &original_valid
//     // ));
// // }

// // #[test]
// // fn stepwise_short_two_frames_offset_unidir() {
// //     let cfg: AppConfig = AppConfigBuilder::default()
// //         .with_scan_period(Period::from_freq(100_000.0))
// //         .with_columns(10)
// //         .with_rows(10)
// //         .with_planes(1)
// //         .with_bidir(Bidirectionality::Unidir)
// //         .with_frame_dead_time(10_000_000)
// //         .with_line_ch(9)
// //         .build();
// //     let (mut window, mut app) = setup(SHORT_TWO_FRAME_BATCH_STREAM, Some(cfg), Some(100));
// //     window.render_with_state(&mut app);
//     // window.close();
//     // // app.step();
//     // let original_invalid: Vec<TimeCoordPair> =
//     //     from_str(&read_to_string("tests/data/short_two_frames_batch_unidir_invalid.ron").unwrap())
//     //         .unwrap();
//     // let original_valid: Vec<TimeCoordPair> =
//     //     from_str(&read_to_string("tests/data/short_two_frames_batch_unidir_valid.ron").unwrap())
//     //         .unwrap();
//     // assert!(timecoordpair_vec_compare(
//     //     &app.invalid_events,
//     //     &original_invalid
//     // ));
//     // assert!(timecoordpair_vec_compare(
//     //     &app.valid_events,
//     //     &original_valid
//     // ));
// // }

// #[test]
// fn offset_with_lines() {
//     let cfg: AppConfig = AppConfigBuilder::default()
//         .with_planes(1)
//         .with_rows(2)
//         .with_columns(2)
//         .with_pmt1_ch(InputChannel::new(-3, 0.0))
//         .with_pmt2_ch(InputChannel::new(8, 0.0))
//         .with_line_ch(InputChannel::new(1, 0.0))
//         .build();
//     let mut app = setup(WITH_LINES_STREAM, Some(cfg));
//     app.start_acq_loop_for(1).unwrap();
//     // to_writer_pretty(File::create("tests/data/record_batch_with_lines.ron").unwrap(), &app.channels[ChannelNames::ChannelMerge], PrettyConfig::new()).unwrap();
//     let original: PointLogger = from_reader(File::open("tests/data/record_batch_with_lines.ron").unwrap()).unwrap();
//     assert_eq!(original, app.channels[ChannelNames::ChannelMerge]);
// }
