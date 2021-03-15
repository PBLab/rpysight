use std::fs::read_to_string;
use std::path::PathBuf;

use pyo3::{prelude::*, types::PyModule}; 

use librpysight::point_cloud_renderer::run_render;
use librpysight::load_timetagger_module;

fn main() -> Result<(), std::io::Error> {
    // Set up the Python side
    let filename = PathBuf::from("rpysight/call_timetagger.py");
    let timetagger_module: PyObject = load_timetagger_module(filename)?;
    let gil = Python::acquire_gil();

    // Set up the renderer side
    let (window, app) = setup_renderer(timetagger_module);

    // Start the TT inside the app and render the photons
    let parsed_data = timetagger_module.call0(gil.python())?;
    window.render_loop(app);
    Ok(())
}
