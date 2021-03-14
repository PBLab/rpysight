use pyo3::{prelude::*, types::{PyModule, PyDict}};
use std::fs::read_to_string;

fn main() {
    let gil = Python::acquire_gil();
    let py = gil.python();
    let python_code = read_to_string("tests/numpy_test.py").expect("No numpy array file found");
    let testarr = PyModule::from_code(py, &python_code, "testing.py", "testarr").expect("fdsf");
    let arr = testarr.getattr("a").expect("No array in file");
    println!("{:?}", arr);
}
