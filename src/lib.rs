//! Real time parsing and rendering of data coming from a TimeTagger

mod photon;
pub mod point_cloud_renderer;
mod interval_tree;
mod rendering_helpers;

use std::path::PathBuf;
use std::fs::read_to_string;

use pyo3::prelude::*;
///
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
    let tt_starter = run_tt.getattr("run_tagger")?;
    // Generate an owned object to be returned by value
    Ok(tt_starter.to_object(py))
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
