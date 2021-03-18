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
import numpy as np
import pyarrow as pa

import TimeTagger
from librpysight import process_stream

# Channel definitions
CHAN_START = 1
CHAN_STOP = 2
TT_DATA_STREAM = '__tt_data_stream.dat'


class CustomTT(TimeTagger.CustomMeasurement):
    """
    Example for a single start - multiple stop measurement.
        The class shows how to access the raw time-tag stream.
    """

    # @classmethod
    # def from_existing_tagger(cls):
        # tagger = TimeTagger.createTimeTagger()

        # enable the test signal
        # tagger.setTestSignal([CHAN_START, CHAN_STOP], True)
        # delay the stop channel by 2 ns to make sure it is later than the start
        # tagger.setInputDelay(CHAN_STOP, 2000)


        # return cls(tagger, CHAN_STOP, CHAN_START)

    def __init__(self, tagger, channels: list):
        TimeTagger.CustomMeasurement.__init__(self, tagger)
        for channel in channels:
            self.register_channel(channel)
        self.schema = pa.schema(('type_', pa.uint8()), ('missed_events', pa.uint16()), ('channel', pa.int32()), ('time', pa.int64()))
        self.stream = pa.ipc.new_stream(TT_DATA_STREAM, self.schema)
        # At the end of a CustomMeasurement construction,
        # we must indicate that we have finished.
        self.finalize_init()

    def __del__(self):
        # The measurement must be stopped before deconstruction to avoid
        # concurrent process() calls.
        self.stream.close()
        self.stop()

    def on_start(self):
        # The lock is already acquired within the backend.
        pass

    def on_stop(self):
        # The lock is already acquired within the backend.
        pass

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
        print(f"Python num rows: {len(incoming_tags)}")
        batch = pa.record_batch([incoming_tags['type'], incoming_tags['missed_events'], incoming_tags['channel'], incoming_tags['time']], schema=self.schema)
        self.stream.write(batch)


def run_tagger():
    # c = CustomStartMultipleStop.from_existing_tagger()
    tagger = TimeTagger.createTimeTagger()
    tagger.reset()
    # enable the test signal
    tagger.setTestSignal([CHAN_START, CHAN_STOP], True)
    tag = CustomTT(tagger, [CHAN_START, CHAN_STOP])
    print("setup complete")
    tag.startFor(int(2e12))
    tag.waitUntilFinished()


if __name__ == "__main__":
    run_tagger()
