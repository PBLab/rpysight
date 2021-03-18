// Remember to  $Env:PYTHONHOME = "C:\Users\PBLab\.conda\envs\timetagger\"
// because powershell is to dumb to remember.
use std::path::PathBuf;
use std::fs::File;

use pyo3::prelude::*;
use rand::prelude::*;
use arrow::ipc::reader::StreamReader;

use librpysight::point_cloud_renderer::{setup_renderer, ImageCoor};
use librpysight::load_timetagger_module;

const TT_DATA_STREAM: &'static str = "__tt_data_stream.dat";

fn main() -> Result<(), std::io::Error> {
    // Set up the Python side
    let filename = PathBuf::from("rpysight/call_timetagger.py");
    let timetagger_module: PyObject = load_timetagger_module(filename)?;
    let gil = Python::acquire_gil();

    // Set up the renderer side
    let (mut window, app) = setup_renderer(gil, timetagger_module);
    // Start the TT inside the app and render the photons
    app.tt_module.call0(Python::acquire_gil().python())?;
    // window.render_loop(app);
    let mut point_holder: Vec<ImageCoor> = Vec::with_capacity(10_001);
    let mut rng = rand::thread_rng();
    let white = ImageCoor::new(1.0, 1.0, 1.0);
    let stream = File::open(TT_DATA_STREAM)?;
    let mut stream = StreamReader::try_new(stream).expect("Stream file missing");
    while window.render() {
        if let Some(batch) = stream.next() {
            for _ in 0..batch.unwrap().num_rows() {
                let point = generate_coor(&mut rng);
                window.draw_point(&point, &white);
            }
            // point_holder.push(point);
        } else {
            break
        }
    }
    Ok(())
}

fn generate_coor(rng: &mut ThreadRng) -> ImageCoor {
    let x: f32 = rng.gen::<f32>(); 
    let y: f32 = rng.gen::<f32>();
    let z: f32 = rng.gen::<f32>();
    let point = ImageCoor::new(x, y, z);
    point
}

fn mock_get_data_from_channel() -> Vec<ImageCoor> {
    let mut rng = rand::thread_rng();
    let mut data = Vec::with_capacity(10_000);
    for _ in 0..10_000 {
        data.push(generate_coor(&mut rng));
    }
    data
}
