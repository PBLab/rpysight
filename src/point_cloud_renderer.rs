extern crate kiss3d;

use kiss3d::point_renderer::PointRenderer;
use rand::prelude::*;

use kiss3d::camera::Camera;
use kiss3d::planar_camera::PlanarCamera;
use kiss3d::post_processing::PostProcessingEffect;
use kiss3d::renderer::Renderer;
use kiss3d::window::{State, Window};
use kiss3d::nalgebra::{Dynamic, MatrixSlice, Point3, U1};

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
    use std::fs::read_to_string;

    use pyo3::{prelude::*, types::PyModule};
    use kiss3d::nalgebra::{U10, Scalar, Matrix, ArrayStorage, Dynamic};
    use numpy::Element;
    use nalgebra_numpy::matrix_slice_from_numpy;

    use super::{process_event, U1, EventStream};

    fn generate_event_stream<'a>() -> EventStream<'a> {
        let type_ = get_arr_from_python_file::<u8>(String::from("type_"));
        let missed_events = get_arr_from_python_file::<u16>(String::from("missed_events"));
        let channel = get_arr_from_python_file::<i32>(String::from("channel"));
        let time = get_arr_from_python_file::<i64>(String::from("time"));
        let len = 10;
        EventStream {
            type_,
            missed_events,
            channel,
            time,
            len,
        }
    }

    fn get_arr_from_python_file<T: Scalar + Element>(arr_name: String) -> Matrix<T, U10, U1, ArrayStorage<T, U10, U1>> {
        let gil = Python::acquire_gil();
        let py = gil.python();
        let python_code = read_to_string("tests/numpy_test.py").expect("No numpy array file found");
        let testfile = PyModule::from_code(py, &python_code, "testing.py", "testarr").expect("Couldn't parse file");
        let gil = Python::acquire_gil();
        let arr = testfile.getattr(&arr_name).expect("Array name not found");
        let b = unsafe { matrix_slice_from_numpy::<T, U10, U1>(gil.python(), arr).unwrap().into_owned() };
        b
    }

    #[test]
    fn test_simple_arange() {
        let data = get_arr_from_python_file::<i64>(String::from("simple_arange"));
        assert_eq!(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9], data.data.to_vec());

    }
}
