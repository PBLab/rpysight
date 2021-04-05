# -*- coding: utf-8 -*-
"""Taken directly from Swabian's examples. Written on 132.66.42.158 (laser
room computer) for realtime interaction with the TT. This somehow works
without any PYTHONPATH manipulation - someone has already added the 
'driver\python' directory and it seems stable enough for this POC.

By and large, the 'process' method is automatically called when each batch of
events arrive. The entire data is in the 'incoming_tags' variable, the
self.data array is a buffer that's holding the intermediate results of their
histogramming effort.

Here they called the numbafied function 'fast_process' during each iteration. I
replaced that function with my own mock function defined in lib.rs just to make
it work once, and it did, which is great.
"""
from time import sleep
import pathlib
import logging

import numpy as np
import pyarrow as pa
import pyarrow.compute as pc

import TimeTagger

# Channel definitions
CHAN_START = 1
CHAN_STOP = 2
TT_DATA_STREAM = "__tt_data_stream.dat"


class CustomTT(TimeTagger.CustomMeasurement):
    """
    Example for a single start - multiple stop measurement.
        The class shows how to access the raw time-tag stream.
    """

    def __init__(self, tagger, channels: list, logger=None):
        TimeTagger.CustomMeasurement.__init__(self, tagger)
        for channel in channels:
            self.register_channel(channel)

        self.init_stream_and_schema()

        if logger:
            logger.info("Setup complete")
            self.logger = logger
        # At the end of a CustomMeasurement construction,
        # we must indicate that we have finished.
        self.finalize_init()

    def init_stream_and_schema(self):
        self.schema = pa.record_batch(
            [
                pa.array([], type=pa.uint8()),
                pa.array([], type=pa.uint16()),
                pa.array([], type=pa.int32()),
                pa.array([], type=pa.int64()),
            ],
            names=["type_", "missed_events", "channel", "time"],
        ).schema
        pathlib.Path(TT_DATA_STREAM).unlink(missing_ok=True)
        self.stream = pa.ipc.new_stream(TT_DATA_STREAM, self.schema)

    def __del__(self):
        # The measurement must be stopped before deconstruction to avoid
        # concurrent process() calls.
        self.stop()

    def on_start(self):
        # The lock is already acquired within the backend.
        pass

    def on_stop(self):
        # The lock is already acquired within the backend.
        pass

    def convert_tags_to_recordbatch(self, incoming_tags):
        num_tags = len(incoming_tags)
        type_ = pa.UInt8Array.from_buffers(
            pa.uint8(),
            num_tags,
            [None, pa.py_buffer(np.ascontiguousarray(incoming_tags["type"]))],
            null_count=0,
        )
        missed_events = pa.UInt16Array.from_buffers(
            pa.uint16(),
            num_tags,
            [None, pa.py_buffer(np.ascontiguousarray(incoming_tags["missed_events"]))],
            null_count=0,
        )
        channel = pa.Int32Array.from_buffers(
            pa.int32(),
            num_tags,
            [None, pa.py_buffer(np.ascontiguousarray(incoming_tags["channel"]))],
            null_count=0,
        )
        time = pa.Int64Array.from_buffers(
            pa.int64(),
            num_tags,
            [None, pa.py_buffer(np.ascontiguousarray(incoming_tags["time"]))],
            null_count=0,
        )
        batch = pa.record_batch(
            [type_, missed_events, channel, time],
            schema=self.schema
        )
        return batch

    def convert_tags_to_batch_inefficient(self, incoming_tags):
        batch = pa.array(incoming_tags, type=self.struct)
        batch = pa.record_batch([batch], schema=self.schema)
        return batch

    def process(self, incoming_tags, begin_time, end_time):
        """
        Main processing method for the incoming raw time-tags.

        The lock is already acquired within the backend.
        self.data is provided as reference, so it must not be accessed
        anywhere else without locking the mutex.

        Parameters
        ----------
        incoming_tags
            The incoming raw time tag stream provided as a read-only reference.
            The storage will be deallocated after this call, so you must not store a reference to
            this object. Make a copy instead.
            Please note that the time tag stream of all channels is passed to the process method,
            not only the onces from register_channel(...).
        begin_time
            Begin timestamp of the of the current data block.
        end_time
            End timestamp of the of the current data block.
        """
        batch = self.convert_tags_to_recordbatch(incoming_tags)
        self.stream.write(batch)


def run_tagger():
    # c = CustomStartMultipleStop.from_existing_tagger()
    tagger = TimeTagger.createTimeTagger()
    tagger.reset()
    # enable the test signal
    tagger.setTestSignal([CHAN_START, CHAN_STOP], True)
    FORMAT = '%(levelname)s %(name)s %(asctime)-15s %(filename)s:%(lineno)d %(message)s'
    logging.basicConfig(filename='target/rpysight_timetagger.log', format=FORMAT)
    logger = logging.getLogger().setLevel(logging.INFO)
    tag = CustomTT(tagger, [CHAN_START, CHAN_STOP], logger)


    tag.startFor(int(5e12))
    tag.waitUntilFinished()


if __name__ == "__main__":
    run_tagger()
