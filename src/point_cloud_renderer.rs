extern crate kiss3d;

use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::net::TcpStream;
use std::ops::{Index, IndexMut};
use std::path::Path;
use std::sync::Arc;

use anyhow::{Context, Result};
use arrow2::io::ipc::write::StreamWriter;
use arrow2::{
    array::{Array, Float32Array, StructArray, UInt32Array, UInt8Array},
    datatypes::{
        DataType::{Float32, Struct, UInt32, UInt8},
        Field, Schema,
    },
    io::ipc::read::{read_stream_metadata, StreamReader, StreamState},
    record_batch::RecordBatch,
};
use crossbeam::channel::{unbounded, Receiver};
use hashbrown::HashMap;
use kiss3d::window::Window;
use nalgebra::Point3;
use ordered_float::OrderedFloat;

use crate::configuration::{AppConfig, DataType, Inputs};
use crate::event_stream::{Event, EventStream};
use crate::snakes::{
    Coordinate, Picosecond, Snake, ThreeDimensionalSnake, TwoDimensionalSnake, VoxelDelta,
};
use crate::{COLOR_INCREMENT, DISPLAY_COLORS, GRAYSCALE_START, GRAYSCALE_STEP};
/// A coordinate in image space, i.e. a float in the range [0, 1].
/// Used for the rendering part of the code, since that's the type the renderer
/// requires.
pub type ImageCoor = Point3<Coordinate>;

/// A handler of streaming time tagger data
pub trait EventStreamHandler {
    fn event_to_coordinate(&mut self, event: Event) -> ProcessedEvent;
    fn get_event_stream<'a>(&mut self, batch: &'a RecordBatch) -> Option<EventStream<'a>>;
}

/// The result of handling an event generated by the time tagger.
///
/// Each event might arrive from different channels which require different
/// handling, and this enum contains all possible actions we might want to do
/// with these results.
#[derive(Debug, Clone, Copy)]
pub enum ProcessedEvent {
    /// Contains the coordinates in image space and the color
    Displayed(Point3<Coordinate>, usize),
    /// Nothing to do with this event
    NoOp,
    /// A new frame signal
    FrameNewFrame,
    /// Start drawing a new frame due to a line signal that belongs to the
    /// next frame (> num_rows)
    LineNewFrame,
    /// Start drawing a new frame due to a photon signal with a time after the
    /// end of the current frame. Probably means that we didn't record all line
    /// signals that arrived during the frame
    PhotonNewFrame,
    /// Erroneuous event, usually for tests
    Error,
}

/// Implemented by Apps who wish to display points
pub trait PointDisplay {
    /// Add the point to the renderer. This is where the ordered_float
    /// abstraction "leaks" and we have to use the native type that the
    /// underlying library expects.
    fn display_point(&mut self, p: &ImageCoor, c: &Point3<f32>, time: Picosecond);
    /// Start the GPU-based rendering process
    fn render(&mut self);
    /// Hide the rendering window
    fn hide(&mut self);
    /// Whether the acquisition is over and we may stop acquisition and
    /// rendering
    fn should_close(&self) -> bool;
}

#[derive(Clone, Copy, Debug)]
pub struct Channels<T: PointDisplay> {
    channel1: T,
    channel2: T,
    channel3: T,
    channel4: T,
    channel_merge: T,
}

impl<T: PointDisplay> Channels<T> {
    pub fn new(mut channels: Vec<T>) -> Self {
        assert!(channels.len() == 5);
        Self {
            channel1: channels.remove(0),
            channel2: channels.remove(0),
            channel3: channels.remove(0),
            channel4: channels.remove(0),
            channel_merge: channels.remove(0),
        }
    }

    pub fn hide_all(&mut self) {
        self.channel1.hide();
        self.channel2.hide();
        self.channel3.hide();
        self.channel4.hide();
        self.channel_merge.hide();
    }

    /// Render all channels.
    ///
    /// Due to issues with kiss3d we only render a single channel - the merged one -
    /// at this time.
    pub fn render(
        &mut self,
        frame_buffers: &mut [HashMap<Point3<OrderedFloat<f32>>, Point3<f32>>],
    ) {
        // Channels::render_single_channel(&mut frame_buffers[0], &mut self.channel1);
        // Channels::render_single_channel(&mut frame_buffers[1], &mut self.channel2);
        // Channels::render_single_channel(&mut frame_buffers[2], &mut self.channel3);
        // Channels::render_single_channel(&mut frame_buffers[3], &mut self.channel4);
        Channels::render_single_channel(&mut frame_buffers[4], &mut self.channel_merge);
        // Drain the other ununsed channels
        frame_buffers[0].clear();
        frame_buffers[1].clear();
        frame_buffers[2].clear();
        frame_buffers[3].clear();
    }

    fn render_single_channel(
        frame_buffer: &mut HashMap<Point3<OrderedFloat<f32>>, Point3<f32>>,
        channel: &mut T,
    ) {
        frame_buffer
            .drain()
            .for_each(|(k, v)| channel.display_point(&k, &v, 0));
        channel.render();
    }

    pub fn should_close(&self) -> bool {
        // We'll use channel_merge as the indicator because it will always be
        // used
        self.channel_merge.should_close()
    }
}

pub enum ChannelNames {
    Channel1,
    Channel2,
    Channel3,
    Channel4,
    ChannelMerge,
}

impl<T: PointDisplay> Index<ChannelNames> for Channels<T> {
    type Output = T;

    fn index(&self, index: ChannelNames) -> &Self::Output {
        match index {
            ChannelNames::Channel1 => &self.channel1,
            ChannelNames::Channel2 => &self.channel2,
            ChannelNames::Channel3 => &self.channel3,
            ChannelNames::Channel4 => &self.channel4,
            ChannelNames::ChannelMerge => &self.channel_merge,
        }
    }
}

impl<T: PointDisplay> IndexMut<ChannelNames> for Channels<T> {
    fn index_mut(&mut self, index: ChannelNames) -> &mut Self::Output {
        match index {
            ChannelNames::Channel1 => &mut self.channel1,
            ChannelNames::Channel2 => &mut self.channel2,
            ChannelNames::Channel3 => &mut self.channel3,
            ChannelNames::Channel4 => &mut self.channel4,
            ChannelNames::ChannelMerge => &mut self.channel_merge,
        }
    }
}

/// Holds the custom renderer that will be used for rendering the
/// point cloud
pub struct DisplayChannel {
    pub window: Window,
}

impl PointDisplay for DisplayChannel {
    #[inline]
    fn display_point(&mut self, p: &ImageCoor, c: &Point3<f32>, _time: Picosecond) {
        // Convert the point to ScanImage's FOV (and to f32)
        let p0: &Point3<f32> = &Point3::new(-*p.y, -*p.x, *p.z);
        self.window.draw_point(p0, c)
    }

    fn render(&mut self) {
        self.window.render();
    }

    fn hide(&mut self) {
        self.window.hide();
    }

    fn should_close(&self) -> bool {
        self.window.should_close()
    }
}

impl DisplayChannel {
    pub fn new(title: &str, width: u32, height: u32, frame_rate: u64) -> Self {
        let mut window = Window::new_with_size(title, width, height);
        window.set_framerate_limit(Some(frame_rate));
        Self { window }
    }
}

/// Main struct that holds the renderers and the needed data streams for
/// them
pub struct AppState<T: PointDisplay, R: Read> {
    pub channels: Channels<T>,
    data_stream_fh: String,
    pub data_stream: Option<StreamReader<R>>,
    snake: Box<dyn Snake>,
    inputs: Inputs,
    rows_per_frame: u32,
    line_count: u32,
    lines_vec: Vec<Picosecond>,
    batch_readout_count: u64,
    frame_buffers: [HashMap<Point3<OrderedFloat<f32>>, Point3<f32>>; 5],
}

impl<T: PointDisplay, R: Read> AppState<T, R> {
    /// Generates a new app from a renderer and a receiving end of a channel
    pub fn new(channels: Channels<T>, data_stream_fh: String, appconfig: AppConfig) -> Self {
        let snake = AppState::<T, R>::choose_snake_variant(&appconfig);
        AppState {
            channels,
            data_stream_fh,
            data_stream: None,
            snake,
            inputs: Inputs::from_config(&appconfig),
            rows_per_frame: appconfig.rows,
            line_count: 0,
            lines_vec: Vec::<Picosecond>::with_capacity(3000),
            batch_readout_count: 0,
            frame_buffers: [
                HashMap::with_capacity(600_000),
                HashMap::with_capacity(600_000),
                HashMap::with_capacity(1),
                HashMap::with_capacity(1),
                HashMap::with_capacity(600_000),
            ],
        }
    }

    /// Decide on 2D or 3D rendering based on the configuration.
    fn choose_snake_variant(config: &AppConfig) -> Box<dyn Snake + 'static> {
        match config.planes {
            0 | 1 => Box::new(TwoDimensionalSnake::from_acq_params(config, 0)),
            2..=u32::MAX => Box::new(ThreeDimensionalSnake::from_acq_params(config, 0)),
        }
    }

    /// Render the data to the screen
    fn render(&mut self) {
        self.channels.render(&mut self.frame_buffers);
    }

    /// Called when an event from the line channel arrives to the event stream.
    ///
    /// It handles the first line of the experiment, by returning a special
    /// signal, a standard line in the middle of the frame or a line which
    /// is the first in the next frame's line count.
    fn handle_line_event(&mut self, time: Picosecond) -> ProcessedEvent {
        // The new line that arrived is the first of the next frame
        if self.line_count == self.rows_per_frame {
            self.line_count = 0;
            debug!("Here are the lines: {:#?}", self.lines_vec);
            self.lines_vec.clear();
            self.snake.update_snake_for_next_frame(time);
            ProcessedEvent::LineNewFrame
        } else {
            self.line_count += 1;
            self.lines_vec.push(time);
            ProcessedEvent::NoOp
        }
    }

    /// Called when an event from the frame channel arrives
    fn handle_frame_event(&mut self, time: Picosecond) -> ProcessedEvent {
        debug!("A new frame due to a frame signal");
        self.line_count = 0;
        self.lines_vec.clear();
        self.snake.update_snake_for_next_frame(time);
        ProcessedEvent::FrameNewFrame
    }

    /// One of the main functions of the app, responsible for iterating over
    /// data streams.
    ///
    /// It receives the leftover events from the previous analyzed batch and
    /// starts processing it. Once it's done it can read a new batch of data
    /// and process it in the same manner.
    ///
    /// The iteration on the event batches is done in a way that lets us
    /// "remember" the last location on the batch that we visited. The method
    /// "find_map" mutates the iterator so that when we re-visit it we start
    /// at the next event in line, which is very efficient.
    pub fn populate_single_frame(
        &mut self,
        events_after_newframe: Option<Vec<Event>>,
    ) -> Option<Vec<Event>> {
        if let Some(previous_events) = events_after_newframe {
            debug!("Looking for leftover events");
            // Start with the leftover events from the previous frame
            let mut previous_events_mut = previous_events.iter();
            let new_frame_in_pre_events =
                previous_events_mut.find_map(|event| self.act_on_single_event(*event));
            if let Some(_) = new_frame_in_pre_events {
                return Some(previous_events_mut.copied().collect::<Vec<Event>>());
            }
        };
        // New experiments will start out here, by loading the data and
        // looking for the first line signal
        debug!("Starting a 'frame loop");
        loop {
            // The following lines cannot be factored to a function due to
            // borrowing - the data stream contains a reference to 'batch', so
            // 'batch' cannot go out of scope
            let batch = match self.data_stream.as_mut().unwrap().next() {
                Some(batch) => match batch {
                    Ok(b) => match b {
                        StreamState::Some(x) => {
                            self.batch_readout_count += 1;
                            x
                        }
                        StreamState::Waiting => {
                            debug!("Waiting on new stream");
                            continue;
                        }
                    },
                    Err(b) => {
                        error!(
                            "In populate, batch couldn't be extracted. Num: {}, error: {:?}",
                            self.batch_readout_count, b
                        );
                        continue;
                    }
                },
                None => {
                    debug!("End of stream",);
                    break None;
                }
            };
            let event_stream = match self.get_event_stream(&batch) {
                Some(stream) => stream,
                None => {
                    debug!("Couldn't get event stream");
                    continue;
                }
            };
            let mut leftover_event_stream = event_stream.iter();
            match self.check_relevance_of_batch(&event_stream) {
                true => {}
                false => {
                    debug!("Batch irrelevant!");
                    continue;
                }
            };
            info!("Starting iteration on this stream");
            // Main iteration on events from this current batch
            let new_frame_found =
                leftover_event_stream.find_map(|event| self.act_on_single_event(event));
            // If this batch contained a new frame - we return the leftovers
            if let Some(_) = new_frame_found {
                debug!("New frame found in the batch!");
                return Some(leftover_event_stream.collect::<Vec<Event>>());
            }
            info!("Let's loop again, we're still inside a single frame");
        }
    }

    /// Verifies that the current event stream lies within the boundaries of
    /// the current frame we're trying to render.
    fn check_relevance_of_batch(&self, event_stream: &EventStream) -> bool {
        if let Some(event) = Event::from_stream_idx(&event_stream, event_stream.num_rows() - 1) {
            if event.time <= self.snake.get_earliest_frame_time() {
                debug!("The last event in the batch arrived before the first in the frame: received event: {}, earliest in frame: {}", event.time ,self.snake.get_earliest_frame_time());
                false
            } else {
                true
            }
        } else {
            error!("For some reason no last event exists in this stream");
            false
        }
    }

    /// Adds the point with its color to a pixel list that will be drawn in the
    /// next rendering pass.
    /// The method is agnostic to the coordinate and the color it has,
    /// rather its job is to increment the color of the that pixel if this
    /// isn't the first time a photon has arrived at that pixel. Else it gives
    /// that pixel its default color.
    ///
    /// Each individual color channel is rendered in grayscale since they're
    /// separate, and thus they're incremented using [`GRAYSALE_STEP`]. But the
    /// merged channel shows each channel with its respective color, so this
    /// channel, marked as `frame_buffers[4]` is using a different incrementing
    /// method.
    ///
    /// Due to limitations of kiss3d all frame_buffers others than the 4th one
    /// (merge) aren't rendered, but their photons are still added to these
    /// buffers because they'll be used in the serialization process later on.
    fn add_to_render_queue(&mut self, point: ImageCoor, channel: usize) {
        self.frame_buffers[channel]
            .entry(point)
            .and_modify(|c| c.apply(|d| d + GRAYSCALE_STEP))
            .or_insert(*GRAYSCALE_START);
        self.frame_buffers[4]
            .entry(point)
            .and_modify(|c| c.apply(|d| d * COLOR_INCREMENT))
            .or_insert(DISPLAY_COLORS[channel]);
    }

    /// The function called on each event in the processed batch.
    ///
    /// It first finds what type of event has it received (a photon that needs
    /// rendering, a line event, etc.) and then acts on it accordingly. The
    /// return value is necessary to fulfil the demands of "find_map" which
    /// halts only when Some(val) is returned.
    fn act_on_single_event(&mut self, event: Event) -> Option<i8> {
        match self.event_to_coordinate(event) {
            ProcessedEvent::Displayed(point, channel) => {
                self.add_to_render_queue(point, channel);
                None
            }
            ProcessedEvent::NoOp => None,
            ProcessedEvent::FrameNewFrame => {
                info!("New frame due to frame signal");
                Some(0)
            }
            ProcessedEvent::PhotonNewFrame => {
                info!(
                    "New frame due to photon {} while we had {} lines",
                    event.time, self.line_count
                );
                Some(0)
            }
            ProcessedEvent::LineNewFrame => {
                info!("New frame due to line");
                Some(0)
            }
            ProcessedEvent::Error => {
                error!("Received an erroneuous event: {:?}", event);
                None
            }
        }
    }

    /// Returns the event stream only from the first event after the first line
    /// of the frame.
    ///
    /// When it finds the first line it also updates the internal state of this
    /// object with this knowledge.
    fn advance_till_first_frame_line(
        &mut self,
        event_stream: Option<Vec<Event>>,
    ) -> Option<Vec<Event>> {
        if let Some(previous_events) = event_stream {
            info!("Looking for the first line/frame in the previous event stream");
            let mut previous_events_mut = previous_events.iter();
            let mut steps = 0;
            let frame_started =
                previous_events_mut.find_map(|event| {
                    if event.type_ == 0 {
                        match self.inputs.get(event.channel) {
                            DataType::Line => Some((DataType::Line, event.time)),
                            DataType::Frame => Some((DataType::Frame, event.time)),
                            _ => {
                                steps += 1;
                                None
                            }
                        }
                    } else {
                        warn!("Overflow: {:?}", event);
                        None
                    }
                });
            if let Some(started) = frame_started {
                self.lines_vec.clear();
                match started.0 {
                    DataType::Line => {
                        self.line_count = 1;
                    }
                    DataType::Frame => {
                        self.line_count = 0;
                    }
                    _ => {}
                }
                info!(
                    "Found the first line/frame in the previous event stream ({}) after {} steps",
                    started.1, steps
                );
                self.snake.update_snake_for_next_frame(started.1);
                return Some(previous_events_mut.copied().collect::<Vec<Event>>());
            };
        }
        // We'll look for the first line\frame indefinitely
        loop {
            // The following lines cannot be factored to a function due to
            // borrowing - the data stream contains a reference to 'batch', so
            // 'batch' cannot go out of scope
            let batch = match self.data_stream.as_mut().unwrap().next() {
                Some(batch) => match batch {
                    Ok(b) => match b {
                        StreamState::Some(x) => {
                            self.batch_readout_count += 1;
                            x
                        }
                        StreamState::Waiting => {
                            debug!("Waiting on new stream");
                            continue;
                        }
                    },
                    Err(b) => {
                        error!(
                            "Couldn't extract batch from stream ({}): {:?}",
                            self.batch_readout_count, b
                        );
                        continue;
                    }
                },
                None => continue,
            };
            let event_stream = match self.get_event_stream(&batch) {
                Some(stream) => stream,
                None => {
                    info!("No stream found, restarting loop");
                    continue;
                }
            };
            let mut leftover_event_stream = event_stream.iter();
            info!("Looking for the first line/frame in a newly acquired stream");
            let frame_started = leftover_event_stream
                .find_map(|event| {
                    if event.type_ == 0 {
                        match self.inputs.get(event.channel) {
                        &DataType::Line => Some((DataType::Line, event.time)),
                        &DataType::Frame => Some((DataType::Frame, event.time)),
                        &DataType::Invalid => {
                            error!("Out of bounds access: {:?}", event);
                            None
                        }
                        _ => None,
                        }
                    } else {
                        warn!("Overflow: {:?}", event);
                        None
                    }
                });
            if let Some(started) = frame_started {
                self.lines_vec.clear();
                match started.0 {
                    DataType::Frame => self.line_count = 0,
                    DataType::Line => self.line_count = 1,
                    _ => {}
                }
                info!("Found the first line/frame: {}", started.1);
                self.snake.update_snake_for_next_frame(started.1);
                return Some(leftover_event_stream.collect::<Vec<Event>>());
            }
        }
    }
}

impl<T: PointDisplay> AppState<T, TcpStream> {
    /// Main loop of the app. Following a bit of a setup, during each frame
    /// loop we advance the photon stream iterator until the first line event,
    /// and then we iterate over all of the photons of that frame, until we
    /// detect the last of the photons or a new frame signal.
    pub fn start_inf_acq_loop(&mut self, config: AppConfig) -> Result<()> {
        self.acquire_stream_filehandle()?;
        let mut events_after_newframe = self.advance_till_first_frame_line(None);
        let mut frame_number = 1usize;
        let rolling_avg = config.rolling_avg as usize;
        let (sender, receiver) = unbounded();
        let voxel_delta = self.snake.get_voxel_delta_im();
        let handle =
            std::thread::spawn(move || serialize_data(receiver, voxel_delta, config.filename));
        while !self.channels.should_close() {
            info!("Starting the population of single frame");
            events_after_newframe = self.populate_single_frame(events_after_newframe);
            if frame_number % rolling_avg == 0 {
                sender.send(self.frame_buffers.clone()).unwrap();
                debug!("Starting render");
                self.render();
            };
            frame_number += 1;
            events_after_newframe = self.advance_till_first_frame_line(events_after_newframe);
        }
        info!("Writing to disk");
        drop(sender);
        handle.join().unwrap();
        Ok(())
    }

    /// Instantiate an IPC StreamReader using an existing file handle.
    fn acquire_stream_filehandle(&mut self) -> Result<()> {
        if self.data_stream.is_none() {
            std::thread::sleep(std::time::Duration::from_secs(9));
            debug!("Finished waiting");
            let mut reader = TcpStream::connect(&self.data_stream_fh)
                .context("Can't open stream file, exiting.")?;
            let meta = read_stream_metadata(&mut reader).context("Can't read stream metadata")?;
            let stream = StreamReader::new(reader, meta);
            self.data_stream = Some(stream);
            debug!("File handle for stream acquired!");
        } else {
            debug!("File handle already acquired.");
        }
        Ok(())
    }
}

impl<T: PointDisplay> AppState<T, File> {
    /// Instantiate an IPC StreamReader using an existing file handle.
    ///
    /// Used for testing purposes.
    pub fn acquire_filehandle(&mut self) -> Result<()> {
        if self.data_stream.is_none() {
            let mut reader =
                File::open(&self.data_stream_fh).context("Can't open stream file, exiting.")?;
            let meta = read_stream_metadata(&mut reader).context("Can't read stream metadata")?;
            let stream = StreamReader::new(reader, meta);
            self.data_stream = Some(stream);
            debug!("File handle for stream acquired!");
        } else {
            debug!("File handle already acquired.");
        }
        Ok(())
    }

    /// Main loop of the app. Following a bit of a setup, during each frame
    /// loop we advance the photon stream iterator until the first line event,
    /// and then we iterate over all of the photons of that frame, until we
    /// detect the last of the photons or a new frame signal.
    pub fn start_acq_loop_for(&mut self, steps: usize, rolling_avg: u16) -> Result<()> {
        self.acquire_filehandle()?;
        let mut events_after_newframe = self.advance_till_first_frame_line(None);
        let rolling_avg = rolling_avg as usize;
        let mut frame_number = 1usize;
        for _ in 0..steps {
            debug!("Starting population");
            events_after_newframe = self.populate_single_frame(events_after_newframe);
            if frame_number % rolling_avg == 0 {
                debug!("Calling render");
                self.channels.channel_merge.render();
            };
            frame_number += 1;
            events_after_newframe = self.advance_till_first_frame_line(events_after_newframe);
        }
        info!("Acq loop done");
        Ok(())
    }
}

impl<T: PointDisplay, R: Read> EventStreamHandler for AppState<T, R> {
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
        trace!("Received the following event: {:?}", event);
        match self.inputs[event.channel] {
            DataType::Pmt1 => self.snake.time_to_coord_linear(event.time, 0),
            DataType::Pmt2 => self.snake.time_to_coord_linear(event.time, 1),
            DataType::Pmt3 => self.snake.time_to_coord_linear(event.time, 2),
            DataType::Pmt4 => self.snake.time_to_coord_linear(event.time, 3),
            DataType::Line => self.handle_line_event(event.time),
            DataType::TagLens => self.snake.new_taglens_period(event.time),
            DataType::Laser => self.snake.new_laser_event(event.time),
            DataType::Frame => self.handle_frame_event(event.time),
            DataType::Unwanted => ProcessedEvent::NoOp,
            DataType::Invalid => {
                warn!("Unsupported event: {:?}", event);
                ProcessedEvent::NoOp
            }
        }
    }

    /// Generates an EventStream instance from the loaded record batch
    #[inline]
    fn get_event_stream<'b>(&mut self, batch: &'b RecordBatch) -> Option<EventStream<'b>> {
        debug!(
            "When generating the EventStream we received {} rows",
            batch.num_rows()
        );
        let event_stream = EventStream::from_streamed_batch(batch);
        if event_stream.num_rows() == 0 {
            info!("A batch with 0 rows was received");
            None
        } else {
            Some(event_stream)
        }
    }
}

/// Write the data to disk in a tabular format.
///
/// This function will take the per-frame data, convert it to a clearer
/// serialization format and finally write it to disk.
fn serialize_data<P: AsRef<Path>>(
    recv: Receiver<[HashMap<Point3<OrderedFloat<f32>>, Point3<f32>>; 5]>,
    voxel_delta: VoxelDelta<Coordinate>,
    filename: P,
) {
    let mut coord_to_index = match CoordToIndex::try_new(&voxel_delta, filename) {
        Ok(cti) => cti,
        Err(e) => {
            error!(
                "Cannot create a file: {:?}. Not writing columnar data to disk",
                e
            );
            return;
        }
    };
    loop {
        match recv.recv() {
            Ok(new_data) => {
                let (channels, xs, ys, zs, colors) = coord_to_index.map_data_to_indices(new_data);
                let rb = coord_to_index.convert_vecs_to_recordbatch(channels, xs, ys, zs, colors);
                match coord_to_index.serialize_to_stream(rb) {
                    Ok(()) => {},
                    Err(e) => {
                        error!("Failed to serialize: {:?}", e);
                    }
                };
            }
            Err(_) => break,
        };
    }
    coord_to_index.stream.finish().unwrap();
}

/// Convert the GPU-focused coordinates to array indexing.
///
/// We wish to have access to the GPU array that is rendered in each step, but
/// since that's impossible we use this struct to create a proxy - a mapping
/// between the GPU-based coordinates (probably in the range [-0.5, 0.5]) to
/// array indices ([0..len]).
struct CoordToIndex {
    row_mapping: BTreeMap<OrderedFloat<f32>, u32>,
    column_mapping: BTreeMap<OrderedFloat<f32>, u32>,
    plane_mapping: BTreeMap<OrderedFloat<f32>, u32>,
    stream: StreamWriter<File>,
    schema: Arc<Schema>,
}

impl CoordToIndex {
    /// Try to create a new mapping from the voxel delta information
    pub fn try_new<P: AsRef<Path>>(
        voxel_delta: &VoxelDelta<Coordinate>,
        filename: P,
    ) -> Result<Self> {
        let (row, col, plane) = voxel_delta.map_coord_to_index();
        info!(
            "Got the following mapping: Row: {:#?}, Col: {:#?}, Plane: {:#?}",
            row, col, plane
        );
        let schema = Schema::new(vec![
            Field::new("channel", UInt8, false),
            Field::new("x", UInt32, false),
            Field::new("y", UInt32, false),
            Field::new("z", UInt32, false),
            Field::new(
                "color",
                Struct(vec![
                    Field::new("r", Float32, false),
                    Field::new("g", Float32, false),
                    Field::new("b", Float32, false),
                ]),
                false,
            ),
        ]);
        let f = File::create(filename.as_ref().with_extension("arrow_stream"))?;
        info!("Writing the table to disk at: {:?}", f);
        let stream = StreamWriter::try_new(f, &schema)?;
        Ok(Self {
            row_mapping: row,
            column_mapping: col,
            plane_mapping: plane,
            stream,
            schema: Arc::new(schema),
        })
    }

    /// Convert the GPU-based coordinates and brightness levels to a table of
    /// array-focused coordinates.
    pub fn map_data_to_indices(
        &self,
        data: [HashMap<Point3<OrderedFloat<f32>>, Point3<f32>>; 5],
    ) -> (Vec<u8>, Vec<u32>, Vec<u32>, Vec<u32>, Vec<Point3<f32>>) {
        let length = data[4].len();
        let mut channels = Vec::<u8>::with_capacity(length);
        let mut xs = Vec::<u32>::with_capacity(length);
        let mut ys = Vec::<u32>::with_capacity(length);
        let mut zs = Vec::<u32>::with_capacity(length);
        let mut colors = Vec::<Point3<f32>>::with_capacity(length);
        for (ch, single_channel_data) in data.iter().enumerate() {
            for (point, color) in single_channel_data.iter() {
                debug!("Point to push: {:?}", point);
                let r = match self.row_mapping.get(&point.x) {
                    Some(r) => *r,
                    None => continue,
                };
                let c = match self.column_mapping.get(&point.y) {
                    Some(c) => *c,
                    None => continue,
                };
                let p = match self.plane_mapping.get(&point.z) {
                    Some(p) => *p,
                    None => continue,
                };
                // All points are not NaNs, we can add them to the buffers
                channels.push(ch as u8);
                xs.push(r);
                ys.push(c);
                zs.push(p);
                colors.push(*color);
            }
        }
        (channels, xs, ys, zs, colors)
    }

    /// Convert the "raw" table of data into a [`RecordBatch`] that can be
    /// streamed and serialized.
    pub fn convert_vecs_to_recordbatch(
        &self,
        channels: Vec<u8>,
        xs: Vec<u32>,
        ys: Vec<u32>,
        zs: Vec<u32>,
        colors: Vec<Point3<f32>>,
    ) -> RecordBatch {
        let channels = Arc::new(UInt8Array::from_trusted_len_values_iter(channels.into_iter()));
        let xs = Arc::new(UInt32Array::from_trusted_len_values_iter(xs.into_iter()));
        let ys = Arc::new(UInt32Array::from_trusted_len_values_iter(ys.into_iter()));
        let zs = Arc::new(UInt32Array::from_trusted_len_values_iter(zs.into_iter()));
        let colors = self.convert_colors_vec_to_arrays(colors);
        let iter_over_vecs: Vec<Arc<dyn Array>> = vec![
            channels,
            xs,
            ys,
            zs,
            colors,
        ];
        RecordBatch::try_new(self.schema.clone(), iter_over_vecs).unwrap()
    }

    /// Create the specific structure of the colors (=brightness) to an Arrow-
    /// centered data representation.
    pub fn convert_colors_vec_to_arrays(&self, colors: Vec<Point3<f32>>) -> Arc<StructArray> {
        let length = colors.len();
        let mut colors_x = Vec::<f32>::with_capacity(length);
        let mut colors_y = Vec::<f32>::with_capacity(length);
        let mut colors_z = Vec::<f32>::with_capacity(length);
        for p in colors {
            colors_x.push(p.x);
            colors_y.push(p.y);
            colors_z.push(p.z);
        }
        let colors_x = Arc::new(Float32Array::from_trusted_len_values_iter(colors_x.into_iter()));
        let colors_y = Arc::new(Float32Array::from_trusted_len_values_iter(colors_y.into_iter()));
        let colors_z = Arc::new(Float32Array::from_trusted_len_values_iter(colors_z.into_iter()));
        let colors = Arc::new(StructArray::from_data(Struct(
            vec![
                Field::new("r", arrow2::datatypes::DataType::Float32, false),
                Field::new("g", arrow2::datatypes::DataType::Float32, false),
                Field::new("b", arrow2::datatypes::DataType::Float32, false),
            ]),
            vec![colors_x, colors_y, colors_z],
            None,
        ));
        colors
    }

    /// Write the data to disk
    pub fn serialize_to_stream(&mut self, rb: RecordBatch) -> Result<()> {
        self.stream.write(&rb)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::configuration::{AppConfigBuilder, Bidirectionality, InputChannel, Period};
    use crate::snakes::*;
    use nalgebra::Point3;
    use std::env::temp_dir;

    fn setup_default_config() -> AppConfigBuilder {
        AppConfigBuilder::default()
            .with_point_color(Point3::new(1.0f32, 1.0, 1.0))
            .with_rows(256)
            .with_columns(256)
            .with_planes(10)
            .with_scan_period(Period::from_freq(7926.17))
            .with_tag_period(Period::from_freq(189800))
            .with_bidir(Bidirectionality::Bidir)
            .with_fill_fraction(71.3)
            .with_frame_dead_time(8 * *Period::from_freq(7926.17))
            .with_pmt1_ch(InputChannel::new(-1, 0.0))
            .with_pmt2_ch(InputChannel::new(0, 0.0))
            .with_pmt3_ch(InputChannel::new(0, 0.0))
            .with_pmt4_ch(InputChannel::new(0, 0.0))
            .with_laser_ch(InputChannel::new(0, 0.0))
            .with_frame_ch(InputChannel::new(0, 0.0))
            .with_line_ch(InputChannel::new(2, 0.0))
            .with_taglens_ch(InputChannel::new(3, 0.0))
            .with_line_shift(0)
            .clone()
    }

    fn create_mock_maps() -> [HashMap<Point3<OrderedFloat<f32>>, Point3<f32>>; 5] {
        let mut map = HashMap::new();
        map.insert(
            Point3::<OrderedFloat<f32>>::new(
                OrderedFloat(0.5),
                OrderedFloat(0.3),
                OrderedFloat(0.0),
            ),
            Point3::<f32>::new(0.1, 0.1, 0.2),
        );
        let map2 = HashMap::new();
        let map3 = map2.clone();
        let map4 = map2.clone();
        let map5 = map.clone();
        [map, map2, map3, map4, map5]
    }

    fn create_record_batch() -> RecordBatch {
        todo!()
    }

    fn read_data_stream<P: AsRef<Path>>(filename: P) -> RecordBatch {
        todo!()
    }

    #[test]
    fn serialize_data_2d() {
        let (sender, receiver) = unbounded();
        let mut filename = temp_dir();
        filename.push("test_serialize.test");
        let fname = filename.clone();
        let voxel_delta = VoxelDelta::<Coordinate>::from_config(&setup_default_config().build());
        std::thread::spawn(move || serialize_data(receiver, voxel_delta, &filename));
        let base_data = create_mock_maps();
        sender.send(base_data).unwrap();
        let truth_recordbatch = create_record_batch();
        let streamed_data = read_data_stream(&fname);
        assert_eq!(truth_recordbatch, streamed_data);
    }
}
