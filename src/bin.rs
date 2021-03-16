// Remember to  $Env:PYTHONHOME = "C:\Users\PBLab\.conda\envs\timetagger\"
// because powershell is to dumb to remember.
use std::path::PathBuf;

use pyo3::prelude::*;

use librpysight::point_cloud_renderer::setup_renderer;
use librpysight::load_timetagger_module;

fn main() -> Result<(), std::io::Error> {
    // Set up the Python side
    let filename = PathBuf::from("rpysight/call_timetagger.py");
    let timetagger_module: PyObject = load_timetagger_module(filename)?;
    let gil = Python::acquire_gil();

    // Set up the renderer side
    let (window, app) = setup_renderer(gil, timetagger_module);
    // Start the TT inside the app and render the photons
    // let parsed_data = app.tt_module.call0(Python::acquire_gil().python())?;
    window.render_loop(app);
    Ok(())
}
