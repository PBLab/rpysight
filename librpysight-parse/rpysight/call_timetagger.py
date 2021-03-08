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
import matplotlib.pyplot as plt
import numpy as np
import numba

import TimeTagger
# from librpysight import process_stream

# Channel definitions
CHAN_START = 1
CHAN_STOP = 2


class CustomStartMultipleStop(TimeTagger.CustomMeasurement):
    """
    Example for a single start - multiple stop measurement.
        The class shows how to access the raw time-tag stream.
    """

    @classmethod
    def from_existing_tagger(cls):
        tagger = TimeTagger.createTimeTagger()

        # enable the test signal
        tagger.setTestSignal([CHAN_START, CHAN_STOP], True)
        # delay the stop channel by 2 ns to make sure it is later than the start
        tagger.setInputDelay(CHAN_STOP, 2000)

        BINWIDTH = 1  # ps
        BINS = 4000

        return cls(tagger, CHAN_STOP, CHAN_START, binwidth=BINWIDTH, n_bins=BINS)

    def __init__(self, tagger, click_channel, start_channel, binwidth, n_bins):
        TimeTagger.CustomMeasurement.__init__(self, tagger)
        self.click_channel = click_channel
        self.start_channel = start_channel
        self.binwidth = binwidth
        self.max_bins = n_bins

        # The method register_channel(channel) activates
        # that data from the respective channels is transferred
        # from the Time Tagger to the PC.
        self.register_channel(channel=click_channel)
        self.register_channel(channel=start_channel)

        self.clear_impl()

        # At the end of a CustomMeasurement construction,
        # we must indicate that we have finished.
        self.finalize_init()

    def __del__(self):
        # The measurement must be stopped before deconstruction to avoid
        # concurrent process() calls.
        self.stop()

    def getData(self):
        # Acquire a lock this instance to guarantee that process() is not running in parallel
        # This ensures to return a consistent data.
        self._lock()
        arr = self.data.copy()
        # We have gathered the data, unlock, so measuring can continue.
        self._unlock()
        return arr

    def getIndex(self):
        # This method does not depend on the internal state, so there is no
        # need for a lock.
        arr = np.arange(0, self.max_bins) * self.binwidth
        return arr

    def clear_impl(self):
        # The lock is already acquired within the backend.
        self.last_start_timestamp = 0
        self.data = np.zeros((self.max_bins,), dtype=np.uint64)

    def on_start(self):
        # The lock is already acquired within the backend.
        pass

    def on_stop(self):
        # The lock is already acquired within the backend.
        pass

    @staticmethod
    @numba.jit(nopython=True, nogil=True)
    def fast_process(
            tags,
            data,
            click_channel,
            start_channel,
            binwidth,
            last_start_timestamp):
        """
        A precompiled version of the histogram algorithm for better performance
        nopython=True: Only a subset of the python syntax is supported.
                       Avoid everything but primitives and numpy arrays.
                       All slow operation will yield an exception
        nogil=True:    This method will release the global interpreter lock. So
                       this method can run in parallel with other python code
        """
        for tag in tags:
            # tag.type can be: 0 - TimeTag, 1- Error, 2 - OverflowBegin, 3 -
            # OverflowEnd, 4 - MissedEvents
            if tag['type'] != 0:
                # tag is not a TimeTag, so we are in an error state, e.g. overflow
                last_start_timestamp = 0
            elif tag['channel'] == click_channel and last_start_timestamp != 0:
                # valid event
                index = (tag['time'] - last_start_timestamp) // binwidth
                if index < data.shape[0]:
                    data[index] += 1
            if tag['channel'] == start_channel:
                last_start_timestamp = tag['time']
        return last_start_timestamp

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
        # self.last_start_timestamp = CustomStartMultipleStop.fast_process(
        #     incoming_tags,
        #     self.data,
        #     self.click_channel,
        #     self.start_channel,
        #     self.binwidth,
        #     self.last_start_timestamp)
        # process_stream(len(incoming_tags), incoming_tags.type, incoming_tags.missed_events, incoming_tags.channel, incoming_tags.time)
        print(len(incoming_tags))

if __name__ == '__main__':
    CustomStartMultipleStop.from_existing_tagger()
