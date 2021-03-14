extern crate kiss3d;
extern crate nalgebra as na;

use std::ops::Index;

use kiss3d::point_renderer::PointRenderer;
use rand::prelude::*;

use kiss3d::camera::Camera;
use kiss3d::planar_camera::PlanarCamera;
use kiss3d::post_processing::PostProcessingEffect;
use kiss3d::renderer::Renderer;
use kiss3d::window::{State, Window};
use na::{Dynamic, MatrixSlice, Point3, U1};

pub type ImageCoor = Point3<f32>;

#[derive(Debug)]
pub(crate) struct Event {
    type_: u8,
    missed_event: u16,
    channel: i32,
    time: i64,
}

impl Event {
    pub(crate) fn new(type_: u8, missed_event: u16, channel: i32, time: i64) -> Self {
        Event {
            type_,
            missed_event,
            channel,
            time,
        }
    }
}

pub(crate) struct EventStreamIter<'a> {
    stream: EventStream<'a>,
    idx: usize,
}

impl<'a> Iterator for EventStreamIter<'a> {
    type Item = Event;

    fn next(&mut self) -> Option<Event> {
        if self.idx >= self.stream.len {
            return None;
        };
        let row = (self.idx, 0usize);
        let cur_row = Event::new(
            self.stream.type_[row],
            self.stream.missed_events[row],
            self.stream.channel[row],
            self.stream.time[row],
        );
        self.idx += 1;
        Some(cur_row)
    }
}

#[derive(Debug, Clone)]
pub(crate) struct EventStream<'a> {
    type_: MatrixSlice<'a, u8, Dynamic, U1, Dynamic, Dynamic>,
    missed_events: MatrixSlice<'a, u16, Dynamic, U1, Dynamic, Dynamic>,
    channel: MatrixSlice<'a, i32, Dynamic, U1, Dynamic, Dynamic>,
    time: MatrixSlice<'a, i64, Dynamic, U1, Dynamic, Dynamic>,
    len: usize,
}


impl<'a> EventStream<'a> {
    pub(crate) fn iter(self) -> EventStreamIter<'a> {
        EventStreamIter { stream: self, idx: 0usize }
    }
}

impl<'a> IntoIterator for EventStream<'a> {
    type Item = Event;
    type IntoIter = EventStreamIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

// Holds the custom renderer that will be used for rendering the
// point cloud
struct AppState {
    point_cloud_renderer: PointRenderer,
}

impl AppState {
    // Generates a new app from a renderer and a receiving end of a channel
    pub fn new(point_cloud_renderer: PointRenderer) -> Self {
        AppState {
            point_cloud_renderer,
        }
    }

    pub fn mock_get_data_from_channel(&self) -> Vec<Point3<f32>> {
        let mut rng = rand::thread_rng();
        let mut data = Vec::with_capacity(10_000);
        for i in 1..10_000 {
            let x: f32 = rng.gen();
            let y: f32 = rng.gen();
            let z: f32 = rng.gen();
            let point = Point3::new(x, y, z);
            data[i] = point;
        }
        data
    }

    fn get_event_stream<'a>(&self) -> EventStream<'a> {
        todo!()
    }
}

fn process_event(event: Event) -> Option<ImageCoor> {
    println!("{:?}", event);
    Some(ImageCoor::new(1.0, 0.0, 2.0))
}

impl State for AppState {
    // Return the renderer that will be called at each render loop. Without
    // returning it the loop still runs but the screen is blank.
    fn cameras_and_effect_and_renderer(
        &mut self,
    ) -> (
        Option<&mut dyn Camera>,
        Option<&mut dyn PlanarCamera>,
        Option<&mut dyn Renderer>,
        Option<&mut dyn PostProcessingEffect>,
    ) {
        (None, None, Some(&mut self.point_cloud_renderer), None)
    }

    // Main logic per step - required by the State trait. The function reads
    // data awaiting in the channel and draws each of these points
    // individually.
    fn step(&mut self, window: &mut Window) {
        let white = Point3::new(1.0, 1.0, 1.0);
        let event_stream = self.get_event_stream();
        for event in event_stream {
            if let Some(point) = process_event(event) {
                self.point_cloud_renderer.draw_point(point, white);
            }
        }
    }
}

pub fn run_render() {
    let window = Window::new("RPySight");
    let app = AppState::new(PointRenderer::new());

    window.render_loop(app)
}

#[cfg(test)]
mod tests {
    extern crate numpy;
    extern crate pyo3;
    use std::io;
    use std::fs::read_to_string;

    use pyo3::{prelude::*, types::{PyModule, PyDict}};
    use super::{process_event, EventStream};
    use nalgebra::{Dim, DimName, Dynamic, Matrix, DMatrix, MatrixSlice, Scalar, SliceStorage, U1, U10};
    use numpy::Element;
    use nalgebra_numpy::matrix_slice_from_numpy;

    unsafe fn gen_matrix_slice_from_numpy<'a, N, R, C>(
        array: Vec<N>,
    ) -> MatrixSlice<'a, N, R, C, Dynamic, Dynamic>
    where
        N: Scalar + Element,
        R: Dim,
        C: Dim,
    {
        let mat = DMatrix::from_vec(array.len(), 1, array.clone());
        // let row_stride = Dynamic::new(mat.strides().0 as usize / std::mem::size_of::<N>());
        let row_stride = Dynamic::new(array.len() * std::mem::size_of::<N>());
        // let col_stride = Dynamic::new(mat.strides().1 as usize / std::mem::size_of::<N>());
        let col_stride = Dynamic::new(std::mem::size_of::<N>());
        let shape = (R::from_usize(array.len()), C::from_usize(1));
        let storage = SliceStorage::<N, R, C, Dynamic, Dynamic>::from_raw_parts(
            mat.as_ptr(),
            shape,
            (row_stride, col_stride),
        );
        Matrix::from_data(storage)
    }

    fn generate_event_stream<'a>() -> EventStream<'a> {
        let len: usize = 10;
        let type_ = unsafe { gen_matrix_slice_from_numpy(vec![0u8; len]) };
        let missed_events = unsafe { gen_matrix_slice_from_numpy(vec![1u16; len]) };
        let channel = unsafe { gen_matrix_slice_from_numpy(vec![2i32; len]) };
        let time = unsafe { gen_matrix_slice_from_numpy(vec![3i64; len]) };
        EventStream {
            type_,
            missed_events,
            channel,
            time,
            len,
        }
    }

    fn get_numpy_test_array<'a, N, R, C>() -> MatrixSlice<'a, N, R, C, Dynamic, Dynamic> 
    where N: Scalar + Element,
    R: Dim,
    C: Dim {
       let gil = Python::acquire_gil();
       let py = gil.python();
       let python_code = read_to_string("tests/numpy_test.py").expect("No numpy array file found");
       let testarr = PyModule::from_code(py, &python_code, "numpy_test.py", "testarr").expect("fdsf");
       let arr = testarr.getattr("arr").expect("No array in file");
       matrix_slice_from_numpy(py, arr).unwrap()

    }

    #[test]
    fn test_event_stream() {
        // let st = generate_event_stream();
        let arr = get_numpy_test_array();
        for event in st {
            process_event(event);
        }
        println!("hi");
    }
}
