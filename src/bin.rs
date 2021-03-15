use pyo3::{prelude::*, types::PyModule}; 
use std::fs::read_to_string;

fn main() -> Result<(), std::io::Error> {
    let gil = Python::acquire_gil();
    let py = gil.python();
    let python_code = read_to_string("rpysight/__init__.py").expect("No TimeTagger class file");
    println!("{}", python_code);
    let run_tt = PyModule::from_code(py, &python_code, "run_tt.py", "run_tt")?;
    println!("OK");
    // let tt_starter = run_tt.getattr("CustomStartMultipleStop").expect("Class not found");
    // let existing = tt_starter.getattr("from_existing_tagger").expect("Class method not found").call0().expect("Failed to call the class");
    Ok(())
}
