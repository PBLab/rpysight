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
import socket
from time import sleep

import numpy as np
import pyarrow as pa
import toml

try:
    import TimeTagger
except ModuleNotFoundError:
    import sys
    sys.path.append(r"C:\Users\PBLab\.conda\envs\timetagger")
    import TimeTagger


TT_DATA_STREAM = "tt_data_stream.arrow_stream"
HOST = '127.0.0.1'
PORT = 64444


class RealTimeRendering(TimeTagger.CustomMeasurement):
    """Process the photon stream live and render it in 2D/3D.

    This class streams the live tag data arriving from the TimeTagger to an
    external Rust app named rPySight that parses the individual events and
    renders them in 2D or 3D.

    This class is always instantiated by that Rust process and should not be
    used independently of it.
    """

    def __init__(self, tagger, channels: Optional[list], fname: Optional[str] = None):
        super().__init__(tagger)
        # if channels:
        #     [tagger.setTriggerLevel(ch['channel'], ch['threshold']) for ch in channels]
        if channels:
            [self.register_channel(channel=channel['channel']) for channel in channels]
        self.init_stream_and_schema()
        if fname:
            self.filehandle = open(fname, "wb")

        # At the end of a CustomMeasurement construction,
        # we must indicate that we have finished.
        self.finalize_init()

    def init_stream_and_schema(self):
        test_batch = pa.record_batch(
            [
                pa.array([], type=pa.uint8()),
                pa.array([], type=pa.uint16()),
                pa.array([], type=pa.int32()),
                pa.array([], type=pa.int64()),
            ],
            names=["type_", "missed_events", "channel", "time"],
        )
        self.schema = test_batch.schema
        print("Starting socketing")

        self.socket = socket.socket()
        print("got me socketing")
        self.socket.bind((HOST, PORT))
        print("Bound")
        self.socket.listen()
        print("listening")
        self.conn, addr = self.socket.accept()
        print("accepted")
        self.socketfile = self.conn.makefile('wb')
        print("makefile done")
        opts = pa.ipc.IpcWriteOptions(allow_64bit=True)
        self.stream = pa.ipc.new_stream(self.socketfile, self.schema, options=opts)
        print('self.stream done')
        self.stream.write(test_batch)

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
            [type_, missed_events, channel, time], schema=self.schema
        )
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
        # Saving the data to an npy file for future-proofing purposes
        # np.save(self.filehandle, incoming_tags)


def infer_channel_list_from_cfg(config):
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


def add_fname_suffix(fname: str) -> str:
    fname_path = pathlib.Path(fname)
    new_name = fname_path.stem + "_stream.ttbin"
    return str(fname_path.with_name(new_name))


def run_tagger(cfg: str):
    """Run a TimeTagger acquisition with the GUI's parameters.

    This function starts an acquisition using parameters from the rPySight GUI.
    It should be called from that GUI since the parameter it receives arrives
    directly from Rust.

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
        _ = RealTimeRendering(measure_group.getTagger(), channels, config['filename'])
        measure_group.startFor(int(1_000_000e12))
        measure_group.waitUntilFinished()
    print("TimeTagger has turned the measurement off")


def replay_existing(cfg: str):
    """A testing method to replay old acquisitions."""
    config = toml.loads(cfg)
    tagger = TimeTagger.createTimeTaggerVirtual()
    _ = RealTimeRendering(tagger, None, None)
    tagger.replay(config['filename'], queue=False)
    tagger.waitForCompletion(timeout=-1)


if __name__ == '__main__':
    print("Imports are working")
