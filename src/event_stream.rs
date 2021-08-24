//! Objects and functions that deal directly with the data stream
//! from the TimeTagger.

use std::fmt::Debug;
use std::fs::File;

use anyhow::{Context, Result};
use arrow2::array::{Array, Int32Array, Int64Array, UInt16Array, UInt8Array};
use arrow2::datatypes::DataType;
use arrow2::record_batch::RecordBatch;
use arrow2::io::ipc::read::{read_stream_metadata, StreamReader};
use arrow2::error::ArrowError;
use lazy_static::lazy_static;
use pyo3::prelude::*;

lazy_static! {
    static ref TYPE_: UInt8Array = UInt8Array::new_empty(DataType::UInt8);
    static ref MISSED_EVENTS: UInt16Array = UInt16Array::new_empty(DataType::UInt16);
    static ref CHANNEL: Int32Array = Int32Array::new_empty(DataType::Int32);
    static ref TIME: Int64Array = Int64Array::new_empty(DataType::Int64);
    static ref EMPTY_EVENT_STREAM: EventStream<'static> = EventStream {
        type_: &TYPE_,
        missed_events: &MISSED_EVENTS,
        channel: &CHANNEL,
        time: &TIME,
    };
}

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

    /// Populate the `data_stream` attribute of the implementing struct by
    /// opening the filehandle of the stream and asserting that something's
    /// there.
    fn acquire_stream_filehandle(&mut self) -> Result<()>;
    /// Get a consuming iterator that we can parse into the event stream.
    fn get_mut_data_stream(&mut self) -> Option<&mut Self::StreamIterator>;
    /// Generate the `EventStream` struct from the item we're iterating over.
    /// `EventStream` is used in the downstream processing of this data.
    fn get_event_stream<'a>(&mut self, batch: &'a Self::InnerItem) -> Option<EventStream<'a>>;
}

/// An Apache Arrow based data stream.
///
/// Data is streamed using their IPC format - it's converted on the TT side to
/// a pyarrow record batch, and read as a Rust RecordBatch on the other side.
pub struct ArrowIpcStream {
    pub data_stream_fh: String,
    data_stream: Option<StreamReader<File>>,
}

impl ArrowIpcStream {
    pub fn new(data_stream_fh: String) -> Self {
        Self {
            data_stream_fh,
            data_stream: None,
        }
    }
}

impl TimeTaggerIpcHandler for ArrowIpcStream {
    type InnerItem = RecordBatch;
    type IterError = ArrowError;
    type StreamIterator = StreamReader<File>;

    /// Instantiate an IPC StreamReader using an existing file handle.
    fn acquire_stream_filehandle(&mut self) -> Result<()> {
        if self.data_stream.is_none() {
            std::thread::sleep(std::time::Duration::from_secs(11));
            debug!("Finished waiting");
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

    /// Get a consuming iterator.
    fn get_mut_data_stream(&mut self) -> Option<&mut StreamReader<File>> {
        self.data_stream.as_mut()
    }
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
///
/// An non-consuming iterator wrapper for [`EventStream`]
#[derive(Clone, Debug)]
pub struct RefEventStreamIter<'a> {
    stream: &'a EventStream<'a>,
    idx: usize,
    len: usize,
}

impl<'a> Iterator for RefEventStreamIter<'a> {
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

/// An consuming iterator wrapper for [`EventStream`]
#[derive(Clone, Debug)]
pub struct EventStreamIter<'a> {
    pub stream: EventStream<'a>,
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
#[derive(Clone, Debug)]
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

    pub fn empty() -> Self {
        EMPTY_EVENT_STREAM.clone()
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

    pub fn iter(&'a self) -> RefEventStreamIter<'a> {
        self.into_iter()
    }

    pub fn num_rows(&self) -> usize {
        self.type_.len()
    }
}

impl<'a> IntoIterator for EventStream<'a> {
    type Item = Event;
    type IntoIter = EventStreamIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        EventStreamIter {
            len: self.num_rows(),
            stream: self,
            idx: 0usize,
        }
    }
}

impl<'a> IntoIterator for &'a EventStream<'a> {
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
