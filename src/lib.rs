//! Real time parsing and rendering of data coming from a TimeTagger

mod photon;
pub mod point_cloud_renderer;

use std::path::PathBuf;
use std::fs::read_to_string;

use kiss3d::nalgebra::{DVector, Dynamic, U1};
use nalgebra_numpy::matrix_slice_from_numpy;
use pyo3::prelude::*;

use self::photon::ImageCoor;
use point_cloud_renderer::{EventStream, Event};

/// Current state of the app and renderer.
pub struct Context {
    last_line: i64,
    last_line_image_coor: f32,
    last_frame: i64,
    typical_frame_period: i64,
}

impl Context {
    pub(crate) fn new() -> Self {
        Self {
            last_line: 0, last_line_image_coor: 0.0, last_frame: 0, typical_frame_period: 0
        }
    }

    pub(crate) fn set_last_line(&mut self, last_line: i64) -> Option<ImageCoor> {
        self.last_line = last_line;
        self.last_line_image_coor =
            ((self.last_frame - last_line) / self.typical_frame_period) as f32;
        None
    }

    pub(crate) fn set_last_frame(&mut self, last_frame: i64) -> Option<ImageCoor> {
        self.last_frame = last_frame;
        None
    }
}

/// Loads the Python file with the TimeTagger start up script. 
///
/// The given filename should point to a Python file that can run the
/// TimeTagger with a single method call. The returned object will have a 
/// "call0" method that starts the TT.
pub fn load_timetagger_module(fname: PathBuf) -> PyResult<PyObject> {
    let gil = Python::acquire_gil();
    let py = gil.python();
    let python_code = read_to_string(fname)?;
    let run_tt = PyModule::from_code(py, &python_code, "run_tt.py", "run_tt")?;
    let tt_starter = run_tt.getattr("CustomTT")?;
    let from_existing_tagger = tt_starter.getattr("run_tagger")?;
    // Generate an owned object to be returned by value
    Ok(from_existing_tagger.to_object(py))
}

// const CH1: i32 = 1;
// const CH2: i32 = 2;
// const CH_LINE: i32 = 3;
// const CH_FRAME: i32 = 4;


#[pymodule]
fn librpysight<'a>(_py: Python, m: &PyModule) -> PyResult<()> {
    #[pyfn(m, "process_stream")]
    fn convert_py_stream(
        py: Python,
        len: usize,
        type_: &PyAny,
        missed_events: &PyAny,
        channel: &PyAny,
        time: &PyAny,
    ) -> PyResult<Vec<Event>> {
        let type_ = unsafe { matrix_slice_from_numpy::<u8, Dynamic, U1>(py, type_).unwrap() };
        let missed_events =
            unsafe { matrix_slice_from_numpy::<u16, Dynamic, U1>(py, missed_events).unwrap() };
        let channel = unsafe { matrix_slice_from_numpy::<i32, Dynamic, U1>(py, channel).unwrap() };
        let time = unsafe { matrix_slice_from_numpy::<i64, Dynamic, U1>(py, time).unwrap() };
        let stream = EventStream::new(type_, missed_events, channel, time, len);
        let image_coor_vec: Vec<Event> = stream.iter().filter(|a| a.type_ == 0).collect();
        Ok(image_coor_vec)
    }
    Ok(())
}

fn render_stream(stream: Vec<Event>) {
    todo!()
}

fn convert_time_to_coord(
    time: &i64,
    channel: i32,
    coord_vec: &mut Vec<ImageCoor>,
) -> Option<ImageCoor> {
    todo!()
}

#[cfg(test)]
mod tests {
    fn make_tag_vectors() -> (Vec<u8>, Vec<u16>, Vec<i32>, Vec<i64>, Vec<f32>) {
        const SIZE: usize = 100;
        let types = vec![1; SIZE];
        let missed_events = vec![2; SIZE];
        let channels = vec![3; SIZE];
        let times = vec![4; SIZE];
        let results = vec![0f32; SIZE];
        (types, missed_events, channels, times, results)
    }
    #[test]
    fn generate_tags() {
        let (types_, missed_events, channels, times, results) = make_tag_vectors();
        // process_tags(types_, missed_events, channels, times, results);
        todo!()
    }
}
