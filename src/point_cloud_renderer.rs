extern crate kiss3d;

use std::fs::File;
use std::io::Read;

use anyhow::{Context, Result};
use arrow::{
    array::{Int32Array, Int64Array, UInt16Array, UInt8Array},
    ipc::reader::StreamReader,
    record_batch::RecordBatch,
};
use kiss3d::camera::Camera;
use kiss3d::planar_camera::PlanarCamera;
use kiss3d::point_renderer::PointRenderer;
use kiss3d::post_processing::PostProcessingEffect;
use kiss3d::renderer::Renderer;
use kiss3d::window::{State, Window};
use nalgebra::Point3;
use pyo3::prelude::*;
use rand::prelude::*;

use crate::rendering_helpers::{AppConfig, DataType, Inputs, TimeToCoord};
use crate::GLOBAL_OFFSET;

/// A coordinate in image space, i.e. a float in the range [0, 1].
/// Used for the rendering part of the code, since that's the type the renderer
/// requires.
pub type ImageCoor = Point3<f32>;

/// A single tag\event that arrives from the Time Tagger.
#[pyclass]
#[derive(Debug, Copy, Clone)]
pub struct Event {
    pub type_: u8,
    pub missed_event: u16,
    pub channel: i32,
    pub time: i64,
}

impl Event {
    /// Create a new Event with the given values
    pub(crate) fn new(type_: u8, missed_event: u16, channel: i32, time: i64) -> Self {
        Event {
            type_,
            missed_event,
            channel,
            time,
        }
    }

    pub(crate) fn from_stream_idx(stream: &EventStream, idx: usize) -> Self {
        Event {
            type_: stream.type_.value(idx),
            missed_event: stream.missed_events.value(idx),
            channel: stream.channel.value(idx),
            time: stream.time.value(idx),
        }
    }
}

/// An iterator wrapper for [`EventStream`]
pub(crate) struct EventStreamIter<'a> {
    stream: EventStream<'a>,
    idx: usize,
    len: usize,
}

impl<'a> Iterator for EventStreamIter<'a> {
    type Item = Event;

    fn next(&mut self) -> Option<Event> {
        if self.idx < self.len {
            let cur_row = Event::new(
                self.stream.type_.value(self.idx),
                self.stream.missed_events.value(self.idx),
                self.stream.channel.value(self.idx),
                self.stream.time.value(self.idx),
            );
            self.idx += 1;
            Some(cur_row)
        } else {
            None
        }
    }
}

/// A struct of arrays containing data from the TimeTagger.
///
/// Each field is its own array with some specific data arriving via FFI. Since
/// there are only slices here, the main goal of this stream is to provide easy
/// iteration over the tags for the downstream 'user', via the accompanying
/// ['EventStreamIter`].
#[derive(Debug)]
pub(crate) struct EventStream<'a> {
    type_: &'a UInt8Array,
    missed_events: &'a UInt16Array,
    channel: &'a Int32Array,
    time: &'a Int64Array,
}

impl<'a> EventStream<'a> {
    /// Creates a new stream with views over the arriving data.
    pub(crate) fn new(
        type_: &'a UInt8Array,
        missed_events: &'a UInt16Array,
        channel: &'a Int32Array,
        time: &'a Int64Array,
    ) -> Self {
        EventStream {
            type_,
            missed_events,
            channel,
            time,
        }
    }

    pub(crate) fn from_streamed_batch(batch: &'a RecordBatch) -> EventStream<'a> {
        let type_ = batch
            .column(0)
            .as_any()
            .downcast_ref::<UInt8Array>()
            .expect("Type field conversion failed");
        let missed_events = batch
            .column(1)
            .as_any()
            .downcast_ref::<UInt16Array>()
            .expect("Missed events field conversion failed");
        let channel = batch
            .column(2)
            .as_any()
            .downcast_ref::<Int32Array>()
            .expect("Channel field conversion failed");
        let time = batch
            .column(3)
            .as_any()
            .downcast_ref::<Int64Array>()
            .expect("Time field conversion failed");
        EventStream::new(type_, missed_events, channel, time)
    }

    pub(crate) fn iter(self) -> EventStreamIter<'a> {
        EventStreamIter {
            len: self.num_rows(),
            stream: self,
            idx: 0usize,
        }
    }

    pub(crate) fn num_rows(&self) -> usize {
        self.type_.len()
    }
}

impl<'a> IntoIterator for EventStream<'a> {
    type Item = Event;
    type IntoIter = EventStreamIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Holds the custom renderer that will be used for rendering the
/// point cloud and the needed data streams for it
pub(crate) struct AppState<R: Read> {
    point_cloud_renderer: PointRenderer,
    data_stream_fh: String,
    pub data_stream: Option<StreamReader<R>>,
    appconfig: AppConfig,
    time_to_coord: TimeToCoord,
    inputs: Inputs,
}

impl AppState<File> {
    /// Generates a new app from a renderer and a receiving end of a channel
    pub fn new(
        point_cloud_renderer: PointRenderer,
        data_stream_fh: String,
        appconfig: AppConfig,
    ) -> Self {
        AppState {
            point_cloud_renderer,
            data_stream_fh,
            data_stream: None,
            appconfig: appconfig.clone(),
            time_to_coord: TimeToCoord::from_acq_params(&appconfig, GLOBAL_OFFSET),
            inputs: Inputs::from_config(&appconfig),
        }
    }

    pub fn mock_get_data_from_channel(&self, length: usize) -> Vec<ImageCoor> {
        let mut rng = rand::thread_rng();
        let mut data = Vec::with_capacity(10_000);
        for _ in 0..length {
            let x: f32 = rng.gen::<f32>();
            let y: f32 = rng.gen::<f32>();
            let z: f32 = rng.gen::<f32>();
            let point = ImageCoor::new(x, y, z);
            data.push(point);
        }
        data
    }

    pub fn acquire_stream_filehandle(&mut self) -> Result<()> {
        let stream =
            File::open(&self.data_stream_fh).context("Can't open stream file, exiting.")?;
        let stream =
            StreamReader::try_new(stream).context("Stream file missing, cannot recover.")?;
        self.data_stream = Some(stream);
        Ok(())
    }

    /// Convert a raw event tag to a coordinate which will be displayed on the
    /// screen.
    ///
    /// This is the core of the rendering logic of this application, where all
    /// metadata (row, column info) is used to decide where to place a given
    /// event.
    ///
    /// None is returned if the tag isn't a time tag. When the tag is from a
    /// non-imaging channel it's taken into account, but otherwise (i.e. in
    /// cases of overflow it's discarded at the moment.
    pub fn event_to_coordinate(&mut self, event: Event) -> Option<ImageCoor> {
        if event.type_ != 0 {
            return None;
        }
        info!("Received the following event: {:?}", event);
        match self.inputs[event.channel] {
            DataType::Pmt1 => self.time_to_coord.tag_to_coord_linear(event.time),
            DataType::Pmt2 => self.time_to_coord.dump(event.time),
            DataType::Line => self.time_to_coord.new_line(event.time),
            DataType::TagLens => self.time_to_coord.new_taglens_period(event.time),
            DataType::Laser => self.time_to_coord.new_laser_event(event.time),
            _ => {
                error!("Unsupported event: {:?}", event);
                None
            }
        }
    }
}

impl State for AppState<File> {
    /// Return the renderer that will be called at each render loop. Without
    /// returning it the loop still runs but the screen is blank.
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

    /// Main logic per step - required by the State trait. The function reads
    /// data awaiting from the TimeTagger and then pushes it into the renderer.
    /// Each recorded tag (=Event) can be a time tag or a tag signaling
    /// overflow. This iteration process filters these non-time tags from the
    /// more relevant tags.
    fn step(&mut self, _window: &mut Window) {
        if let Some(batch) = self.data_stream.as_mut().unwrap().next() {
            let batch = batch.unwrap();
            // info!("Received {} rows", batch.num_rows());
            // let v = self.mock_get_data_from_channel(batch.num_rows());
            // for p in v {
            //     self.point_cloud_renderer
            //         .draw_point(p, self.appconfig.point_color)
            // }
            let mut idx = 0;
            let event_stream = EventStream::from_streamed_batch(&batch);
            if Event::from_stream_idx(&event_stream, event_stream.num_rows() - 1).time
                <= self.time_to_coord.earliest_frame_time
            {
                info!("The last event in the batch arrived before the first in the frame");
                return;
            }
            for event in event_stream.into_iter() {
                if idx > 10 {
                    break;
                }
                if let Some(point) = self.event_to_coordinate(event) {
                    info!("This point is about to be rendered: {:?}", point);
                    self.point_cloud_renderer
                        .draw_point(point, self.appconfig.point_color)
                }
                idx += 1;
            }
        }
    }
}
