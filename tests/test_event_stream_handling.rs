use std::sync::Arc;
use std::path::PathBuf;
use std::fs::File;
use std::io::{Read, Write};

use arrow::csv::Reader;
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;
use arrow::ipc::{writer::StreamWriter, reader::StreamReader};

use kiss3d::renderer::PointRenderer;
use librpysight::rendering_helpers::{AppConfig, AppConfigBuilder};
use librpysight::point_cloud_renderer::AppState;

/// Run once to generate .dat file which behave as streams
fn test_file_to_stream() {
    let full_batch = "tests/data/real_record_batch.csv";
    let short_batch = "tests/data/real_record_batch_short.csv";
    let schema = Schema::new(vec![
        Field::new("type_", DataType::UInt8, false),
        Field::new("missed_events", DataType::UInt16, false),
        Field::new("channel", DataType::Int32, false),
        Field::new("time", DataType::Int64, false),
    ]);
    let data_as_stream_full = File::create("tests/data/real_record_batch_full_stream.dat").unwrap();
    let data_as_stream_short = File::create("tests/data/real_record_batch_short_stream.dat").unwrap();
    let mut stream_writer_full = StreamWriter::try_new(data_as_stream_full, &schema).unwrap();
    let mut stream_writer_short = StreamWriter::try_new(data_as_stream_short, &schema).unwrap();
    let mut r_full = Reader::new(File::open(full_batch).unwrap(), Arc::new(schema.clone()), true, None, 1024, None, None);
    let mut r_short = Reader::new(File::open(short_batch).unwrap(), Arc::new(schema.clone()), true, None, 1024, None, None);
    stream_writer_full.write(&r_full.next().unwrap().unwrap()).unwrap();
    stream_writer_short.write(&r_short.next().unwrap().unwrap()).unwrap();
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

fn mock_acquisition_loop(cfg: AppConfig) -> AppState<File> {
    let mut fname = PathBuf::new();
    fname.push("tests/data/real_record_batch_short.csv");
    let mut app = AppState::new(PointRenderer::new(), String::from(""), cfg);
    app.data_stream = Some(read_as_stream("tests/data/real_record_batch_full_stream.dat"));
    app

}

fn setup() -> AppState<File> {
    let cfg = AppConfigBuilder::default().build();
    let app = mock_acquisition_loop(cfg);
    app
}

#[test]
fn print() {
    let mut app = setup();
    println!("{:?}", app.data_stream.as_mut().unwrap().next());
}


