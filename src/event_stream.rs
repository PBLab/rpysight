use pyo3::prelude::*;
use lazy_static::lazy_static;
use arrow::{array::{Int32Array, Int64Array, UInt16Array, UInt8Array}, record_batch::RecordBatch};

lazy_static! {
    static ref TYPE_: UInt8Array = UInt8Array::builder(0).finish();
    static ref MISSED_EVENTS: UInt16Array = UInt16Array::builder(0).finish();
    static ref CHANNEL: Int32Array = Int32Array::builder(0).finish();
    static ref TIME: Int64Array = Int64Array::builder(0).finish();

    static ref EMPTY_EVENT_STREAM: EventStream<'static> = EventStream { 
        type_: &TYPE_,
        missed_events: &MISSED_EVENTS,
        channel: &CHANNEL,
        time: &TIME,
    };
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
