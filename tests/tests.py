from time import sleep
from rpysight import call_timetagger
import pyarrow as pa


class MockConfig:
    pass


def make_mock_config():
    config = MockConfig()
    config.pmt1_ch = 0
    config.pmt2_ch = 2
    config.pmt3_ch = -4
    config.pmt4_ch = 0
    config.laser_ch = -11
    config.frame_ch = 0
    config.line_ch = 8
    config.taglens_ch = 0
    return config


def test_infer_channels():
    config = make_mock_config()
    channels = call_timetagger.infer_channel_list_from_cfg(config)
    assert channels == [2, -4, -11, 8]


def test_ipc_works_in_pyarrow():
    buf = "tt_data_stream.dat"
    reader = pa.ipc.open_stream(buf)
    print(reader.schema)
    batches = [b for b in reader]
    print(len(batches))


def test_ipc_written_in_pyarrow():
    data = [
        pa.array([1, 2, 3, 4]),
        pa.array(['foo', 'bar', 'baz', None]),
        pa.array([True, None, False, True])
    ]

    batch = pa.record_batch(data, names=['f0', 'f1', 'f2'])
    sink = pa.NativeFile("tests/data/a.t")
    writer = pa.ipc.new_stream("tests/data/a.t", batch.schema)
    for i in range(10):
        writer.write(batch)

    sleep(2)

    for i in range(10):
        writer.write(batch)

    writer.write(batch[:0])

