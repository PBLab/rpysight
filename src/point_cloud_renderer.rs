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

use crate::configuration::{AppConfig, DataType, Inputs};
use crate::rendering_helpers::{Picosecond, TimeToCoord};
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
    pub fn new(type_: u8, missed_event: u16, channel: i32, time: i64) -> Self {
        Event {
            type_,
            missed_event,
            channel,
            time,
        }
    }

    pub fn from_stream_idx(stream: &EventStream, idx: usize) -> Option<Self> {
        if stream.num_rows() > idx {
            Some(Event {
                type_: stream.type_.value(idx),
                missed_event: stream.missed_events.value(idx),
                channel: stream.channel.value(idx),
                time: stream.time.value(idx),
            })
        } else {
            info!(
                "Accessed idx is out of bounds! Received {}, but length is {}",
                idx,
                stream.num_rows()
            );
            None
        }
    }
}

/// An iterator wrapper for [`EventStream`]
pub struct EventStreamIter<'a> {
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
pub struct EventStream<'a> {
    type_: &'a UInt8Array,
    missed_events: &'a UInt16Array,
    channel: &'a Int32Array,
    time: &'a Int64Array,
}

impl<'a> EventStream<'a> {
    /// Creates a new stream with views over the arriving data.
    pub fn new(
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

    pub fn from_streamed_batch(batch: &'a RecordBatch) -> EventStream<'a> {
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

    pub fn iter(self) -> EventStreamIter<'a> {
        EventStreamIter {
            len: self.num_rows(),
            stream: self,
            idx: 0usize,
        }
    }

    pub fn num_rows(&self) -> usize {
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

/// A handler of streaming time tagger data
pub trait TimeTaggerIpcHandler {
    fn acquire_stream_filehandle(&mut self) -> Result<()>;
    fn event_to_coordinate(&mut self, event: Event) -> ProcessedEvent;
    fn get_event_stream<'a>(&mut self, batch: &'a RecordBatch) -> Option<EventStream<'a>>;
}


/// The result of handling an event generated by the time tagger.
///
/// Each event might arrive from different channels which require different
/// handling, and this enum contains all possible actions we might want to do
/// with these results.
pub enum ProcessedEvent {
    /// Contains the coordinates in image space and the color
    Displayed(Point3<f32>, Point3<f32>),
    /// Nothing to do with this event
    NoOp,
    /// Start drawing a new frame
    NewFrame,
    /// Erroneuous event, usually for tests
    Error,
    /// First line encoutered and its timing
    FirstLine(Picosecond),
}

/// Implemented by Apps who wish to display points
pub trait PointDisplay {
    fn display_point(&mut self, p: Point3<f32>, c: Point3<f32>, time: Picosecond);
}

/// Holds the custom renderer that will be used for rendering the
/// point cloud and the needed data streams for it
pub struct AppState<'a, T: PointDisplay + Renderer, R: Read> {
    pub renderer: T,
    data_stream_fh: String,
    pub data_stream: Option<StreamReader<R>>,
    time_to_coord: TimeToCoord,
    inputs: Inputs,
    appconfig: AppConfig,
    rows_per_frame: u32,
    row_count: u32,
    last_line: Picosecond,
    lines_vec: Vec<Picosecond>,
    previous_event_stream: Option<EventStreamIter<'a>>,
}

impl<'a, T: PointDisplay + Renderer> AppState<'a, T, File> {
    /// Generates a new app from a renderer and a receiving end of a channel
    pub fn new(
        renderer: T,
        data_stream_fh: String,
        appconfig: AppConfig,
    ) -> Self {
        AppState {
            renderer,
            data_stream_fh,
            data_stream: None,
            time_to_coord: TimeToCoord::from_acq_params(&appconfig, GLOBAL_OFFSET),
            inputs: Inputs::from_config(&appconfig),
            appconfig: appconfig.clone(),
            rows_per_frame: appconfig.rows,
            row_count: 0,
            last_line: 0,
            lines_vec: Vec::<Picosecond>::with_capacity(3000),
            previous_event_stream: None,
        }
    }

    /// Called when an event from the line channel arrives to the event stream.
    ///
    /// It handles the first line of the experiment, by returning a special
    /// signal, a standard line in the middle of the frame or a line which
    /// is the first in the next frame's line count.
    fn handle_line_event(&mut self, event: Event) -> ProcessedEvent {
        if self.last_line == 0 {
            self.row_count = 1;
            self.lines_vec.push(event.time);
            self.last_line = event.time;
            info!("Found the first line of the stream: {:?}", event);
            return ProcessedEvent::FirstLine(event.time);
        }
        let time = event.time;
        info!("Elapsed time since last line: {}", time - self.last_line);
        self.last_line = time;
        if self.row_count == self.rows_per_frame {
            self.row_count = 0;
            info!("Here are the lines: {:#?}", self.lines_vec);
            self.lines_vec.clear();
            ProcessedEvent::NewFrame
        } else {
            self.row_count += 1;
            self.lines_vec.push(time);
            ProcessedEvent::NoOp
        }
    }

    /// Verifies that the current event stream lies within the boundaries of
    /// the current frame we're trying to render.
    fn check_relevance_of_batch(&self, event_stream: &EventStream) -> bool {
        if let Some(event) = Event::from_stream_idx(&event_stream, event_stream.num_rows() - 1) {
            if event.time <= self.time_to_coord.earliest_frame_time {
                debug!("The last event in the batch arrived before the first in the frame: received event: {}, earliest in frame: {}", event.time ,self.time_to_coord.earliest_frame_time);
                false
            } else {
                true
            }
        } else {
            error!("For some reason no last event exists in this stream");
            false
        }
    }

    fn find_first_line(&mut self, event: &Event) -> bool {
        match self.event_to_coordinate(*event) {
            ProcessedEvent::FirstLine(time) => {
                self.time_to_coord = TimeToCoord::from_acq_params(&self.appconfig, time);
                true
            }
            _ => false,
        }
    }
}

impl PointDisplay for PointRenderer {
    // #[inline]
    fn display_point(&mut self, p: Point3<f32>, c: Point3<f32>, _time: Picosecond) {
        self.draw_point(p, c);
    }
}

impl<'a, T: PointDisplay + Renderer> TimeTaggerIpcHandler for AppState<'a, T, File> {
    /// Instantiate an IPC StreamReader using an existing file handle.
    fn acquire_stream_filehandle(&mut self) -> Result<()> {
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
    fn event_to_coordinate(&mut self, event: Event) -> ProcessedEvent {
        if event.type_ != 0 {
            warn!("Event type was not a time tag: {:?}", event);
            return ProcessedEvent::NoOp;
        }
        debug!("Received the following event: {:?}", event);
        match self.inputs[event.channel] {
            DataType::Pmt1 => self.time_to_coord.tag_to_coord_linear(event.time, 0),
            DataType::Pmt2 => self.time_to_coord.tag_to_coord_linear(event.time, 1),
            DataType::Pmt3 => self.time_to_coord.tag_to_coord_linear(event.time, 2),
            DataType::Pmt4 => self.time_to_coord.tag_to_coord_linear(event.time, 3),
            DataType::Line => self.handle_line_event(event),
            DataType::TagLens => self.time_to_coord.new_taglens_period(event.time),
            DataType::Laser => self.time_to_coord.new_laser_event(event.time),
            DataType::Frame => ProcessedEvent::NoOp,
            DataType::Invalid => {
                warn!("Unsupported event: {:?}", event);
                ProcessedEvent::NoOp
            }
        }
    }

    #[inline]
    fn get_event_stream<'b>(&mut self, batch: &'b RecordBatch) -> Option<EventStream<'b>> {
        info!("Received {} rows", batch.num_rows());
        let event_stream = EventStream::from_streamed_batch(batch);
        if event_stream.num_rows() == 0 {
            debug!("A batch with 0 rows was received");
            None
        } else {
            Some(event_stream)
        }
    }
}

impl<T: 'static + PointDisplay + Renderer> State for AppState<'static, T, File> {
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
        (None, None, Some(&mut self.renderer), None)
    }

    /// Main logic per step - required by the State trait. The function reads
    /// data awaiting from the TimeTagger and then pushes it into the renderer.
    ///
    /// There are a few checks that are done on the strean before we actuallly
    /// start the rendering process, like whether the events are within the
    /// boundaries of the current frame, or whether there's any data waiting
    /// for us from the time tagger. We also verify that the recorded tags are
    /// indeed time tags and not other types of tags, like overflow tags, which
    /// are currently not handled.
    fn step(&mut self, _window: &mut Window) {
        'step: loop {
            let batch = match self.data_stream.as_mut().unwrap().next() {
                Some(batch) => batch.expect("Couldn't extract batch from stream"),
                None => continue,
            };
            let event_stream = match self.get_event_stream(&batch) {
                Some(stream) => stream,
                None => continue,
            };
            let mut event_stream = event_stream.iter();
            if self.last_line == 0 {
                match event_stream.position(|event| self.find_first_line(&event)) {
                    Some(_) => { },  // .position() advances the iterator for us
                    None => continue,  // we need more data since this batch has no first line
                };
            }
            // match self.check_relevance_of_batch(&event_stream) {
            //     true => {}
            //     false => continue,
            // };
            if let Some(old_stream) = self.previous_event_stream {
                let event_stream = old_stream.chain(event_stream);
            }
            let mut new_frame_found_in_stream = false;
            for event in event_stream {
                match self.event_to_coordinate(event) {
                    ProcessedEvent::Displayed(p, c) => self.renderer.display_point(p, c, event.time),
                    ProcessedEvent::NoOp => continue,
                    ProcessedEvent::NewFrame => {
                        info!("New frame!");
                        // TODO: To test this newframe behavior I'm currently
                        // discarding of all photons in this batch. I'll need
                        // to handle them by saving them in some buffer and
                        // render them in the next frame.
                        self.previous_event_stream = Some(event_stream);
                        break 'step;
                    }
                    ProcessedEvent::FirstLine(time) => {
                        error!("First line already detected! {}", time);
                        continue;
                    }
                    ProcessedEvent::Error => {
                        error!("Received an erroneuous event: {:?}", event);
                        continue;
                    }
                }
            }
        }
    }
}
