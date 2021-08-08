extern crate kiss3d;

use std::ops::{Index, IndexMut};

use anyhow::Result;
use kiss3d::window::Window;
use nalgebra::Point3;

use crate::configuration::{AppConfig, DataType, Inputs};
use crate::event_stream::{Event, EventStream, TimeTaggerIpcHandler};
use crate::snakes::{Picosecond, Snake, ThreeDimensionalSnake, TwoDimensionalSnake};

/// A coordinate in image space, i.e. a float in the range [0, 1].
/// Used for the rendering part of the code, since that's the type the renderer
/// requires.
pub type ImageCoor = Point3<f32>;

/// The result of handling an event generated by the time tagger.
///
/// Each event might arrive from different channels which require different
/// handling, and this enum contains all possible actions we might want to do
/// with these results.
#[derive(Debug, Clone, Copy)]
pub enum ProcessedEvent {
    /// Contains the coordinates in image space and the color
    Displayed(Point3<f32>, Point3<f32>),
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
    fn display_point(&mut self, p: Point3<f32>, c: Point3<f32>, time: Picosecond);
    fn render(&mut self);
    fn hide(&mut self);
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
    fn display_point(&mut self, p: Point3<f32>, c: Point3<f32>, _time: Picosecond) {
        self.window.draw_point(&p, &c)
    }

    fn render(&mut self) {
        self.window.render();
    }

    fn hide(&mut self) {
        self.window.hide();
    }
}

impl DisplayChannel {
    pub fn new(title: &str, width: u32, height: u32, frame_rate: u64) -> Self {
        let mut window = Window::new_with_size(title, width, height);
        window.set_framerate_limit(Some(frame_rate));
        Self { window }
    }

    pub fn get_window(&mut self) -> &mut Window {
        &mut self.window
    }
}

/// Main struct that holds the renderers and the needed data streams for
/// them
pub struct AppState<T: PointDisplay, S: TimeTaggerIpcHandler> {
    pub channels: Channels<T>,
    pub stream: S,
    snake: Box<dyn Snake>,
    inputs: Inputs,
    rows_per_frame: u32,
    line_count: u32,
    lines_vec: Vec<Picosecond>,
    batch_readout_count: u64,
}

impl<T: PointDisplay, S: TimeTaggerIpcHandler> AppState<T, S> {
    /// Generates a new app from a renderer and a receiving end of a channel
    pub fn new(channels: Channels<T>, stream: S, appconfig: AppConfig) -> Self {
        let snake = AppState::<T, S>::choose_snake_variant(&appconfig);
        AppState {
            channels,
            stream,
            snake,
            inputs: Inputs::from_config(&appconfig),
            rows_per_frame: appconfig.rows,
            line_count: 0,
            lines_vec: Vec::<Picosecond>::with_capacity(3000),
            batch_readout_count: 0,
        }
    }

    fn choose_snake_variant(config: &AppConfig) -> Box<dyn Snake + 'static> {
        match config.planes {
            0 | 1 => Box::new(TwoDimensionalSnake::from_acq_params(config, 0)),
            2..=u32::MAX => Box::new(ThreeDimensionalSnake::from_acq_params(config, 0)),
        }
    }

    /// Called when an event from the line channel arrives to the event stream.
    ///
    /// It handles the first line of the experiment, by returning a special
    /// signal, a standard line in the middle of the frame or a line which
    /// is the first in the next frame's line count.
    fn handle_line_event(&mut self, event: Event) -> ProcessedEvent {
        let time = event.time;
        // The new line that arrived is the first of the next frame
        if self.line_count == self.rows_per_frame {
            self.line_count = 0;
            debug!("Here are the lines: {:#?}", self.lines_vec);
            self.lines_vec.clear();
            self.snake.update_snake_for_next_frame(event.time);
            ProcessedEvent::LineNewFrame
        } else {
            self.line_count += 1;
            self.lines_vec.push(time);
            ProcessedEvent::NoOp
        }
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
        // trace!("Received the following event: {:?}", event);
        match self.inputs[event.channel] {
            DataType::Pmt1 => self.snake.time_to_coord_linear(event.time, 0),
            DataType::Pmt2 => self.snake.time_to_coord_linear(event.time, 1),
            DataType::Pmt3 => self.snake.time_to_coord_linear(event.time, 2),
            DataType::Pmt4 => self.snake.time_to_coord_linear(event.time, 3),
            DataType::Line => self.handle_line_event(event),
            DataType::TagLens => self.snake.new_taglens_period(event.time),
            DataType::Laser => self.snake.new_laser_event(event.time),
            DataType::Frame => ProcessedEvent::NoOp,
            DataType::Unwanted => ProcessedEvent::NoOp,
            DataType::Invalid => {
                warn!("Unsupported event: {:?}", event);
                ProcessedEvent::NoOp
            }
        }
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
            let batch = match self.stream.get_mut_data_stream().unwrap().next() {
                Some(batch) => {
                    self.batch_readout_count += 1;
                    batch.expect("Couldn't extract batch from stream")
                }
                None => {
                    debug!(
                        "No batch received for some reason ({})",
                        self.batch_readout_count
                    );
                    continue;
                }
            };
            let event_stream = match self.stream.get_event_stream(&batch) {
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

    /// The function called on each event in the processed batch.
    ///
    /// It first finds what type of event has it received (a photon that needs
    /// rendering, a line event, etc.) and then acts on it accordingly. The
    /// return value is necessary to fulfil the demands of "find_map" which
    /// halts only when Some(val) is returned.
    fn act_on_single_event(&mut self, event: Event) -> Option<i8> {
        match self.event_to_coordinate(event) {
            ProcessedEvent::Displayed(p, c) => {
                self.channels.channel_merge.display_point(p, c, event.time);
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

    /// Main loop of the app. Following a bit of a setup, during each frame
    /// loop we advance the photon stream iterator until the first line event,
    /// and then we iterate over all of the photons of that frame, until we
    /// detect the last of the photons or a new frame signal.
    pub fn start_acq_loop_for(&mut self, steps: usize) -> Result<()> {
        self.stream.acquire_stream_filehandle()?;
        let mut events_after_newframe = None;
        for _ in 0..steps {
            debug!("Starting population");
            events_after_newframe = self.advance_till_first_frame_line(events_after_newframe);
            events_after_newframe = self.populate_single_frame(events_after_newframe);
            debug!("Calling render");
            self.channels.channel_merge.render();
        }
        info!("Acq loop done");
        Ok(())
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
        if let Some(ref previous_events) = event_stream {
            info!("Looking for the first line/frame in the previous event stream");
            let mut steps = 0;
            let frame_started =
                previous_events
                    .iter()
                    .find_map(|event| match self.inputs[event.channel] {
                        DataType::Line | DataType::Frame => Some(event.time),
                        _ => {
                            steps += 1;
                            None
                        }
                    });
            if frame_started.is_some() {
                self.lines_vec.clear();
                self.line_count = 1;
                info!(
                    "Found the first line/frame in the previous event stream ({}) after {} steps",
                    frame_started.unwrap(),
                    steps
                );
                self.snake
                    .update_snake_for_next_frame(frame_started.unwrap());
                return Some(previous_events.iter().copied().collect::<Vec<Event>>());
            };
        }
        // We'll look for the first line\frame indefinitely
        loop {
            // The following lines cannot be factored to a function due to
            // borrowing - the data stream contains a reference to 'batch', so
            // 'batch' cannot go out of scope
            let batch = match self.stream.get_mut_data_stream().unwrap().next() {
                Some(batch) => {
                    self.batch_readout_count += 1;
                    batch.unwrap_or_else(|_| {
                        panic!(
                            "Couldn't extract batch from stream ({})",
                            self.batch_readout_count
                        )
                    })
                }
                None => continue,
            };
            let event_stream = match self.stream.get_event_stream(&batch) {
                Some(stream) => stream,
                None => {
                    info!("No stream found, restarting loop");
                    continue;
                }
            };
            let frame_started =
                event_stream
                    .iter()
                    .find_map(|event| match self.inputs[event.channel] {
                        DataType::Line | DataType::Frame => Some(event.time),
                        _ => None,
                    });
            info!("Looking for the first line/frame in a newly acquired stream");
            if frame_started.is_some() {
                self.lines_vec.clear();
                self.line_count = 1;
                info!("Found the first line/frame: {}", frame_started.unwrap());
                self.snake
                    .update_snake_for_next_frame(frame_started.unwrap());
                return Some(event_stream.iter().collect::<Vec<Event>>());
            }
        }
    }
}

impl<S: TimeTaggerIpcHandler> AppState<DisplayChannel, S> {
    /// Main loop of the app. Following a bit of a setup, during each frame
    /// loop we advance the photon stream iterator until the first line event,
    /// and then we iterate over all of the photons of that frame, until we
    /// detect the last of the photons or a new frame signal.
    pub fn start_inf_acq_loop(&mut self) -> Result<()> {
        self.stream.acquire_stream_filehandle()?;
        let mut events_after_newframe = None;
        while !self.channels.channel_merge.get_window().should_close() {
            events_after_newframe = self.advance_till_first_frame_line(events_after_newframe);
            info!("Starting the population of single frame");
            events_after_newframe = self.populate_single_frame(events_after_newframe);
            debug!("Starting render");
            // self.channel1.render();
            // self.channel2.render();
            // self.channel3.render();
            // self.channel4.render();
            self.channels.channel_merge.render();
        }
        Ok(())
    }
}
