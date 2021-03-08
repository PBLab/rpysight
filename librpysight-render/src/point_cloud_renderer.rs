extern crate kiss3d;
extern crate nalgebra as na;

use kiss3d::point_renderer::PointRenderer;
use rand::prelude::*;

use kiss3d::camera::Camera;
use kiss3d::planar_camera::PlanarCamera;
use kiss3d::post_processing::PostProcessingEffect;
use kiss3d::renderer::Renderer;
use kiss3d::window::{State, Window};
use na::Point3;
use crossbeam_channel::Receiver;


// The event stream provided by the parsing function, which allocate each photon
// to its voxel. Each of the points in the vector is the voxel that point
// belongs to but in image space, i.e. in the range [0.0, 1.0].
type EventStream = Vec<Point3<f32>>;

// Holds the custom renderer that will be used for rendering the
// point cloud
struct AppState {
    point_cloud_renderer: PointRenderer,
    data_rcvr: Receiver<EventStream>,
}

impl AppState {

    // Generates a new app from a renderer and a receiving end of a channel
    pub fn new(point_cloud_renderer: PointRenderer, data_rcvr: Receiver<EventStream>) -> Self {
        AppState { point_cloud_renderer, data_rcvr }
    }
    
    // Call the channel to receive the next volume to render
    pub fn get_data_from_channel(&self) -> EventStream {
        self.data_rcvr.recv().unwrap()
    }

    pub fn mock_get_data_from_channel(&self) -> EventStream {
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
        let points = self.mock_get_data_from_channel();
        for point in points {
            self.point_cloud_renderer.draw_point(point, white);
        };
    }
}

pub fn run_render() {
    let window = Window::new("RPySight");
    let (_, rcvr) = crossbeam_channel::unbounded();
    let app = AppState::new(PointRenderer::new(),  rcvr);

    window.render_loop(app)
}
