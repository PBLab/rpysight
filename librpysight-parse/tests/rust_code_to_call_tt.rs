use pyo3::prelude::*;

fn main() -> PyResult<()> {
    let f = Python::with_gil(|py| {
        let timetagger = PyModule::import(py, "tt_basic_integration")?;
        let total: i32 = timetagger.call_function1("sum", (vec![1, 2, 3],))?.extract()?;
        assert_eq!(total, 6);
        Ok(())
    })
}
