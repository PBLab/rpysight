use std::fs::File;
use arrow2::io::ipc::read::{read_stream_metadata, StreamReader};
use arrow2::io::ipc::write::StreamWriter;
use std::sync::Arc;
use arrow2::array::Int32Array;
use arrow2::datatypes::{Schema, Field, DataType};
use arrow2::record_batch::RecordBatch;

#[test]
fn test_intermittent_writing() {
    // Generate mock data
    let schema = Schema::new(vec![
        Field::new("id", DataType::Int32, false)
    ]);
    let second_schema = schema.clone();

    std::thread::spawn(|| start_writer_and_write(String::from("test_test.d"), second_schema));
    // Let the writer write something
    std::thread::sleep(std::time::Duration::from_secs(1));
    let mut reader = File::open("tt_data_stream.dat").unwrap();
    let meta = read_stream_metadata(&mut reader).unwrap();
    let mut stream = StreamReader::new(&mut reader, meta);
    let mut idx = 0;
    // Start looping and reading the stream. The first loop detects and prints
    // out the array. No other loops detect anything, even though after
    // several seconds new data appears in the stream.
    while idx < 12 {
        match stream.next() {
            Some(_) => {println!("Good");},
            None => println!("None"),
        }
        idx += 1;
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
    println!("Stopped loop, checking whether the data is there");
    println!("{:?}", stream.next().unwrap());  // prints the Batch
    println!("{:?}", stream.next().unwrap());  // also prints the second Batch
}

fn start_writer_and_write(stream_name: String, schema: Schema) {
    let stream = File::create(stream_name).unwrap();
    let mut stream = StreamWriter::try_new(stream, &schema).unwrap();
    let id_array = Int32Array::from_values(vec![1, 2, 3, 4, 5].into_iter());
    let batch = RecordBatch::try_new(
        Arc::new(schema.clone()),
        vec![Arc::new(id_array)]
    ).unwrap();
    stream.write(&batch).unwrap();
    std::thread::sleep(std::time::Duration::from_secs(10));
    stream.write(&batch).unwrap();
    println!("I wrote it, goodbye");
}

#[test]
fn read_previous_data() {
    let mut reader = File::open("tt_data_stream.dat").unwrap();
    let meta = read_stream_metadata(&mut reader).unwrap();
    let mut stream = StreamReader::new(&mut reader, meta);
    let mut idx = 0;
    loop {
        match stream.next() {
            Some(x) => {
                match x {
                    Ok(_) => idx += 1,
                    Err(l) => println!("{:?} ({})", l, idx),
                }
            },
            None => continue,
        };
    }
    println!("{} Done", idx);
}
