//! Objects and functions that deal directly with the data stream
//! from the TimeTagger.

use std::collections::HashMap;
use std::fmt::Debug;

use anyhow::Result;
use crossbeam::channel::Sender;
use nalgebra::DMatrix;
use nalgebra_numpy::matrix_from_numpy;
use pyo3::prelude::*;

use crate::snakes::Picosecond;

/// A protocol for handling the IPC portion of the app.
///
/// This trait's implementers are objects designed to read data from the TT and
/// write it back to the Rust app. This task can be done in several ways, so it
/// was factored out into a trait that can be implemented by the different
/// implementors. The two main examples are the Arrow IPC handler and the TT's
/// own Network class.
pub trait TimeTaggerIpcHandler {
    /// The type the stream has, e.g. RecordBatch
    type InnerItem;
    /// The type of error that creating the iterator may return, e.g. an
    /// ArrowError
    type IterError: Debug;
    /// The iterator's type containing the stream of data, e.g.
    /// StreamReader<File>.
    type StreamIterator: Iterator<Item = Result<Self::InnerItem, Self::IterError>>;

    /// Get a consuming iterator that we can parse into the event stream.
    fn get_mut_data_stream(&mut self) -> Option<&mut Self::StreamIterator>;
    /// Generate the `EventStream` struct from the item we're iterating over.
    /// `EventStream` is used in the downstream processing of this data.
    fn get_event_stream(&mut self, batch: Self::InnerItem) -> Option<EventStream>;
}

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
                type_: stream.type_[(idx, 0)],
                missed_event: stream.missed_events[(idx, 0)],
                channel: stream.channel[(idx, 0)],
                time: stream.time[(idx, 0)],
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
///
/// An non-consuming iterator wrapper for [`EventStream`]
#[derive(Clone, Debug)]
pub struct RefEventStreamIter<'a> {
    stream: &'a EventStream,
    idx: usize,
    len: usize,
}

impl<'a> Iterator for RefEventStreamIter<'a> {
    type Item = Event;

    fn next(&mut self) -> Option<Event> {
        if self.idx < self.len {
            let cur_row = Event::new(
                self.stream.type_[(self.idx, 0)],
                self.stream.missed_events[(self.idx, 0)],
                self.stream.channel[(self.idx, 0)],
                self.stream.time[(self.idx, 0)],
            );
            self.idx += 1;
            Some(cur_row)
        } else {
            None
        }
    }
}

/// An consuming iterator wrapper for [`EventStream`]
#[derive(Clone, Debug)]
pub struct EventStreamIter {
    pub stream: EventStream,
    idx: usize,
    len: usize,
}

impl Iterator for EventStreamIter {
    type Item = Event;

    fn next(&mut self) -> Option<Event> {
        if self.idx < self.len {
            let cur_row = Event::new(
                self.stream.type_[(self.idx, 0)],
                self.stream.missed_events[(self.idx, 0)],
                self.stream.channel[(self.idx, 0)],
                self.stream.time[(self.idx, 0)],
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
#[derive(Clone, Debug)]
pub struct EventStream {
    type_: DMatrix<u8>,
    missed_events: DMatrix<u16>,
    channel: DMatrix<i32>,
    time: DMatrix<Picosecond>,
}

impl EventStream {
    /// Creates a new stream with views over the arriving data.
    pub fn new(
        type_: DMatrix<u8>,
        missed_events: DMatrix<u16>,
        channel: DMatrix<i32>,
        time: DMatrix<Picosecond>,
    ) -> Self {
        EventStream {
            type_,
            missed_events,
            channel,
            time,
        }
    }

    pub fn empty() -> Self {
        Self::from_stream(
            DMatrix::<u8>::from_vec(0, 0, vec![]),
            DMatrix::<u16>::from_vec(0, 0, vec![]),
            DMatrix::<i32>::from_vec(0, 0, vec![]),
            DMatrix::<Picosecond>::from_vec(0, 0, vec![]),
        )
    }

    pub fn from_stream(
        type_: DMatrix<u8>,
        missed_events: DMatrix<u16>,
        channel: DMatrix<i32>,
        time: DMatrix<Picosecond>,
    ) -> EventStream {
        Self {
            type_,
            missed_events,
            channel,
            time,
        }
    }

    pub fn iter<'a>(&'a self) -> RefEventStreamIter<'a> {
        self.into_iter()
    }

    pub fn num_rows(&self) -> usize {
        self.type_.len()
    }
}

impl IntoIterator for EventStream {
    type Item = Event;
    type IntoIter = EventStreamIter;

    fn into_iter(self) -> Self::IntoIter {
        EventStreamIter {
            len: self.num_rows(),
            stream: self,
            idx: 0usize,
        }
    }
}

impl<'a> IntoIterator for &'a EventStream {
    type Item = Event;
    type IntoIter = RefEventStreamIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        RefEventStreamIter {
            stream: self,
            idx: 0usize,
            len: self.num_rows(),
        }
    }
}

pub fn send_arrays_over_ffi(tt_module: Py<PyAny>, sender: Sender<EventStream>, app_config: String) {
    Python::with_gil(|py| {
        let mut tagger = tt_module
            .getattr(py, "TimeTagger")
            .unwrap()
            .getattr(py, "createTimeTagger")
            .unwrap()
            .call0(py)
            .unwrap();
        let mut type_: DMatrix<u8>;
        let mut missed_events: DMatrix<u16>;
        let mut channel: DMatrix<i32>;
        let mut time: DMatrix<Picosecond>;
        debug!("Getting the tagger object from Python!");
        let app_config = tt_module.getattr(py, "toml").unwrap().call_method1(py, "loads", (app_config,)).unwrap();
        let channels = tt_module
            .getattr(py, "infer_channel_list_from_cfg")
            .unwrap()
            .call1(py, (app_config,))
            .unwrap();
        let chan2 = channels.clone();
        tt_module.call_method1(py, "update_tt_triggers", (channels, &tagger)).unwrap();
        let mut previous_begin_time: Picosecond = 0;
        let mut current_begin_time: Picosecond;
        let sync_measure = tt_module
            .getattr(py, "TimeTagger")
            .unwrap()
            .getattr(py, "SynchronizedMeasurements")
            .unwrap()
            .call1(py, (&tagger,))
            .unwrap();
        let measure_group = sync_measure.call_method0(py, "__enter__").unwrap();
        let rt_render = tt_module
            .getattr(py, "RealTimeRendering")
            .unwrap()
            .call1(
                py,
                (
                    measure_group.call_method0(py, "getTagger").unwrap(),
                    chan2,
                    "a.txt".to_string(),
                ),
            )
            .unwrap();
        debug!("About to call start for");
        rt_render
            .call_method1(py, "startFor", (1_000_000e12 as i64,))
            .unwrap();
        loop {
            debug!("Starting FFI loop");
            rt_render.call_method0(py, "waitUntilFinished").unwrap();
            current_begin_time = rt_render
                .getattr(py, "begin_time")
                .unwrap()
                .extract(py)
                .unwrap();
            debug!("RT render: {:?}", rt_render);
            debug!("current_begin_time: {}", current_begin_time);
            if previous_begin_time == current_begin_time {
                debug!("Time hasn't changed, retrying!");
                continue;
            } else {
                trace!("Time has changed");
                previous_begin_time = current_begin_time;
            }
            type_ = matrix_from_numpy(
                py,
                rt_render.getattr(py, "type_").unwrap().extract(py).unwrap(),
            )
            .unwrap();
            missed_events = matrix_from_numpy(
                py,
                rt_render
                    .getattr(py, "missed_events")
                    .unwrap()
                    .extract(py)
                    .unwrap(),
            )
            .unwrap();
            channel = matrix_from_numpy(
                py,
                rt_render
                    .getattr(py, "channel")
                    .unwrap()
                    .extract(py)
                    .unwrap(),
            )
            .unwrap();
            time = matrix_from_numpy(
                py,
                rt_render.getattr(py, "time").unwrap().extract(py).unwrap(),
            )
            .unwrap();
            match sender.send(EventStream::from_stream(
                type_,
                missed_events,
                channel,
                time,
            )) {
                Ok(_) => trace!("Sent batch at time {}", current_begin_time),
                Err(e) => warn!("Error in sending a batch: {:?}", e),
            }
        }
    })
}
