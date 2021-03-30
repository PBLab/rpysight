//! Real time parsing and rendering of data coming from a TimeTagger

mod photon;
pub mod point_cloud_renderer;
mod interval_tree;
mod rendering_helpers;
pub mod gui;

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
