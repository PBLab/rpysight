"""Taken directly from Swabian's examples. Written on 132.66.42.158 (laser
room computer) for realtime interaction with the TT. This somehow works
without any PYTHONPATH manipulation - someone has already added the
'driver\\python' directory and it seems stable enough for this POC.

By and large, the 'process' method is automatically called when each batch of
events arrive. The entire data is in the 'incoming_tags' variable, the
self.data array is a buffer that's holding the intermediate results of their
histogramming effort.

Here they called the numbafied function 'fast_process' during each iteration. I
replaced that function with my own mock function defined in lib.rs just to make
it work once, and it did, which is great.
"""
import pathlib
from typing import Optional

import numpy as np
import pyarrow as pa
import toml

try:
    import TimeTagger
except ModuleNotFoundError:
    import sys
    sys.path.append(r"C:\Users\PBLab\.conda\envs\timetagger")
    import TimeTagger


TT_DATA_STREAM = "tt_data_stream.dat"


class RealTimeRendering(TimeTagger.CustomMeasurement):
    """Process the photon stream live and render it in 2D/3D.

    This class streams the live tag data arriving from the TimeTagger to an
    external Rust app named RPySight that parses the individual events and
    renders them in 2D or 3D.

    This class is always instatiated by that Rust process and should not be
    used independently of it.
    """

    def __init__(self, tagger, channels: Optional[list], fname: Optional[str] = None):
        super().__init__(tagger)
        # if channels:
        #     [tagger.setTriggerLevel(ch['channel'], ch['threshold']) for ch in channels]
        if channels:
            [self.register_channel(channel=channel['channel']) for channel in channels]
        if fname:
            self.filehandle = open(fname, "wb")

        self.begin_time = 0
        # At the end of a CustomMeasurement construction,
        # we must indicate that we have finished.
        self.finalize_init()

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
        if len(incoming_tags) > 0:
            self.type_ = incoming_tags['type']
            self.missed_events = incoming_tags['missed_events']
            self.channel = incoming_tags["channel"]
            self.time = incoming_tags["time"]
            self.begin_time = begin_time
        # Saving the data to an npy file for future-proofing purposes
        # np.save(self.filehandle, incoming_tags)


class TimeTaggerRunner:
    """Runs a TimeTagger experiment via its methods.

    This class constitutes a variety of methods that control different aspects
    of running a TimeTagger experiment. Some of the methods are only loosely
    connected, but we still need them in a single class due to the way we
    interface with the Rust-side and PyO3.
    """
    def __init__(self, cfg: str):
        """
        Parameters
        ----------
        cfg : str
            A TOML string to be parsed into a dictionary
        """
        config = toml.loads(cfg)
        tagger = TimeTagger.createTimeTagger()
        tagger.reset()
        channels = infer_channel_list_from_cfg(config)
        
        if channels:
            [tagger.setTriggerLevel(ch['channel'], ch['threshold']) for ch in channels]
        with TimeTagger.SynchronizedMeasurements(tagger) as measure_group:
            self.tagger = RealTimeRendering(measure_group.getTagger(), channels, config['filename'])
            measure_group.startFor(int(1_000_000e12))
            measure_group.waitUntilFinished()
        print("The TimeTagger has turned the measurement off")


def infer_channel_list_from_cfg(config: dict):
    """Generates a list of channels to register with the TimeTagger based
    on the inputs in the configuration object"""
    relevant_channels = [
        config['pmt1_ch'],
        config['pmt2_ch'],
        config['pmt3_ch'],
        config['pmt4_ch'],
        config['laser_ch'],
        config['frame_ch'],
        config['line_ch'],
        config['taglens_ch'],
    ]
    channels = [ch for ch in relevant_channels if ch["channel"] != 0]
    return channels


def replay_existing(cfg: str):
    """A testing method to replay old acquisitions."""
    config = toml.loads(cfg)
    tagger = TimeTagger.createTimeTaggerVirtual()
    _ = RealTimeRendering(tagger, None, None)
    tagger.replay(config['filename'], queue=False)
    tagger.waitForCompletion(timeout=-1)


def setup_server_tt():
    tagger = TimeTagger.createTimeTagger()
    tagger.reset()
    tagger.setTestSignal(1, True)
    tagger.startServer(51085, [1])
    return tagger


def setup_client_tt():
    tagger = TimeTagger.createTimeTaggerNetwork()
    tagger.connect(domain="127.0.0.1", port=51085)
    return tagger


if __name__ == '__main__':
    server = setup_server_tt()
    tagger = setup_client_tt()
    hist = TimeTagger.Correlation(tagger, 1, binwidth=2, n_bins=2000)
    hist.startFor(int(10e12), clear=True)
    hist.waitUntilFinished()
