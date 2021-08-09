use std::fs::File;
use nalgebra::{DMatrix, DVector, Dynamic, RowDVector, RowVector4, U0, U1, Vector1};
use nalgebra_numpy::{matrix_from_numpy, Error};
use pyo3::{prelude::*, types::PyModule};
use polars::io::ipc::IpcReader;
use polars::io::SerReader;

#[test]
fn pyo3_repeater() {
    Python::with_gil(|py| {
        let activators = PyModule::from_code(py, r#"
from time import sleep
import numpy as np

class C:
    def __init__(self, a):
        self.a = np.array([[a, a, a, a]])
        self.b = np.array([[3.0]])

    def process(self):
        sleep(1)
        self.a += 1
        self.b += 1
"#, "activators.py", "activators").unwrap();
        let c: PyObject = activators.getattr("C").unwrap().call1((-1.0,)).unwrap().extract().unwrap();
        loop {
            c.getattr(py, "process").unwrap().call0(py).unwrap();
            let a: Result<RowVector4<f64>, Error> = matrix_from_numpy(py, c.getattr(py, "a").unwrap().extract(py).unwrap());
            let b = matrix_from_numpy::<f64, U1, U1>(py, c.getattr(py, "b").unwrap().extract(py).unwrap());
            match a {
                Ok(x) => println!("{:?}", x),
                Err(x) => {println!("{:?}", x); continue},
            }
            match b {
                Ok(x) => println!("{:?}", x),
                Err(x) => {println!("{:?}", x); continue},
            }
        }
    })
}

#[test]
fn test_ts() {
    let buf = File::open("target/d.dd").unwrap();
    let df_read = IpcReader::new(&buf).finish().unwrap();
    let df_read2 = IpcReader::new(&buf).finish();
    let df_read3 = IpcReader::new(&buf).finish();

    println!("{:?}", df_read);
    println!("{:?}", df_read2);
    println!("{:?}", df_read3);
    // println!("{:?}", df_read2);
}
