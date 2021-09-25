"""Run the TimeTagger using its Python API.

Taken directly from Swabian's examples. Written on an internal machine (laser
room computer) for real-time interaction with the TT. This somehow works
without any PYTHONPATH manipulation - someone has already added the
'driver\\python' directory and it seems stable enough for this POC.
"""
import pathlib
from typing import Optional
import socket

import numpy as np
import pyarrow as pa
import toml

from TimeTagger import FileWriter, DelayedChannel, GatedChannel

try:
    import TimeTagger
except ModuleNotFoundError:
    import sys
    # Modify this path with your own env path
    sys.path.append(r"C:\Users\PBLab\.conda\envs\timetagger")
    import TimeTagger


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
        if channels:  # during replay we skip it
            [self.register_channel(channel=channel) for channel in channels]
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

        self.socket = socket.socket()
        self.socket.bind((HOST, PORT))
        self.socket.listen()
        self.conn, _ = self.socket.accept()
        self.socketfile = self.conn.makefile('wb')
        opts = pa.ipc.IpcWriteOptions(allow_64bit=True)
        self.stream = pa.ipc.new_stream(
            self.socketfile, self.schema, options=opts
        )
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
        """Transfer the np.ndarray to a RecordBatch.
        
        This is done whenever the TimeTagger sends its data, and this operation
        is quite minimal in its computational requirements.
        
        Note that to construct a pyarrow.Array we need to provide a list with
        two elements, a None and then the actual buffer. I wasn't able to
        understand exactly what's the purpose of the first None (nulls?), but
        regardless it should be kept there.
        """
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
        if len(incoming_tags) > 0:
            batch = self.convert_tags_to_recordbatch(incoming_tags)
            self.stream.write(batch)


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


def set_tt_for_demuxing(tagger, config) -> list:
    """Setup the TimeTagger for demultiplexing.

    This mode is enabled on the user's request, and splits a data channel into
    two or more individual channels, each representing a different temporal
    section of the laser clock period.
    """
    input_channel_to_gate = config[config['demux']['demux_ch']]['channel']
    new_channels = []
    num_demux_channels = config['demux']['periods']
    delay_in_ps = int(config['laser_period']['period'] / num_demux_channels)
    delayed_channels = [DelayedChannel(tagger, config['laser_ch']['channel'], delay_in_ps)]
    for period in range(1, num_demux_channels):
        delayed_channels.append(DelayedChannel(tagger, config['laser_ch']['channel'], delay_in_ps * period))
    for idx, ch in enumerate(delayed_channels[:-1]):
        new_channels.append(GatedChannel(tagger, input_channel_to_gate, ch.getChannel(), delayed_channels[idx + 1].getChannel()))
    new_channels.append(GatedChannel(tagger, input_channel_to_gate, delayed_channels[-1].getChannel(), delayed_channels[0].getChannel()))
    return delayed_channels, new_channels
    

def run_tagger(cfg: str):
    """Run a TimeTagger acquisition with the given parameters.

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
    [tagger.setTriggerLevel(ch['channel'], ch['threshold']) for ch in channels]
    int_channels = [channel['channel'] for channel in channels]
    if config['demux']['demultiplex']:
        _delayed_channels, demux_channels = set_tt_for_demuxing(tagger, config)
        print(_delayed_channels[0].getChannel(), demux_channels[0].getChannel())
        int_channels += [channel.getChannel() for channel in demux_channels]
    with TimeTagger.SynchronizedMeasurements(tagger) as measure_group:
        _rt = RealTimeRendering(measure_group.getTagger(), int_channels, config['filename'])
        _fw = FileWriter(measure_group.getTagger(), config['filename'], int_channels)
        measure_group.startFor(int(1_000_000e12))
        measure_group.waitUntilFinished()


def replay_existing(cfg: str):
    """A testing method to replay old acquisitions."""
    config = toml.loads(cfg)
    tagger = TimeTagger.createTimeTaggerVirtual()
    _ = RealTimeRendering(tagger, None, None)
    tagger.replay(config['filename'], queue=False)
    tagger.waitForCompletion(timeout=-1)
