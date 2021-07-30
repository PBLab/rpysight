use std::fs::File;
use arrow::ipc::reader::StreamReader;

#[test]
fn test_main() {
    let stream = File::open("tt_data_stream.dat").unwrap();
    let mut stream = StreamReader::try_new(stream).unwrap();
    let mut idx = 0;
    loop {
        let batch = stream.next().unwrap().unwrap();
        let num = batch.num_rows();
        if (idx >= 385) && (idx < 400) {
            println!("{:?} after {}", num, idx);
        }
        idx += 1;
    }
}
