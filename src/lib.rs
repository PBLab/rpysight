mod photon;
pub mod point_cloud_renderer;

use kiss3d::nalgebra::{DVector, Dynamic, MatrixSlice, SliceStorage, U1};
use nalgebra_numpy::matrix_slice_from_numpy;
use pyo3::prelude::*;

use self::photon::ImageCoor;
use point_cloud_renderer::{EventStream, EventStreamIter};

pub type ArrivalTimes = DVector<i64>;

struct Context {
    last_line: i64,
    last_line_image_coor: f32,
    last_frame: i64,
    typical_frame_period: i64,
}

impl Context {
    pub(crate) fn new(
        last_line: i64,
        last_line_image_coor: f32,
        last_frame: i64,
        typical_frame_period: i64,
    ) -> Self {
        Self {
            last_line,
            last_line_image_coor,
            last_frame,
            typical_frame_period,
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

const CH1: i32 = 1;
const CH2: i32 = 2;
const CH_LINE: i32 = 3;
const CH_FRAME: i32 = 4;

fn process_tags(
    types: Vec<u8>,
    missed_events: Vec<u16>,
    channels: Vec<i32>,
    times: Vec<i64>,
    resulting_coords: &mut Vec<ImageCoor>,
    context: &mut Context,
) {
    for ((idx, type_), (missed_event, (channel, time))) in types
        .iter()
        .enumerate()
        .zip(missed_events.iter().zip(channels.iter().zip(times.iter())))
    {
        if type_ != &0u8 {
            let coordinate = match channel {
                &CH1 => convert_time_to_coord(time, CH1, resulting_coords),
                &CH2 => convert_time_to_coord(time, CH2, resulting_coords),
                &CH_LINE => context.set_last_line(*time),
                &CH_FRAME => context.set_last_frame(*time),
                _ => panic!(),
            };
            if let Some(coord) = coordinate {
                todo!()
            }
        }
    }
}

#[pymodule]
fn librpysight(py: Python, m: &PyModule) -> PyResult<()> {
    // m.add_wrapped(wrap_pyfunction!(process_stream))?;
    #[pyfn(m, "process_stream")]
    fn convert_py_stream<'a>(
        py: Python,
        len: usize,
        type_: &PyAny,
        missed_events: &PyAny,
        channel: &PyAny,
        time: &'a PyAny,
    ) {
        let type_ = unsafe { matrix_slice_from_numpy::<u8, Dynamic, U1>(py, type_).unwrap() };
        let missed_events =
            unsafe { matrix_slice_from_numpy::<u16, Dynamic, U1>(py, missed_events).unwrap() };
        let channel = unsafe { matrix_slice_from_numpy::<i32, Dynamic, U1>(py, channel).unwrap() };
        let time = unsafe { matrix_slice_from_numpy::<i64, Dynamic, U1>(py, time).unwrap() };
        let stream = EventStream::new(type_, missed_events, channel, time, len);
        println!("{:?}", stream.iter().next());
    }

    Ok(())
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
    use super::process_tags;

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
