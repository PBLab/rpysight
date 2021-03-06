"""
Example of reading an .arrow_stream file generated by rPySight

This file serves as a basic example of reading the stream files that rPysight
writes to disk on the fly. These tabular files need a bit of preprocessing 
before we can extract meaningful, per-channel information out of them. Due to
the streaming format, these files should be read sequentially, with each
iteration corresponding to the displayed volume that rPySight rendered. This
should be considered as a feature, since reading the entire file to memory at
once is probably not needed and perhaps even impossible, due to their sheer
size.

The format of the stream, in the form of a pyarrow.RecordBatch, is the
following: 

color_struct_fields = [
    ("r", pa.float32()),
    ("g", pa.float32()),
    ("b", pa.float32()),
]

schema = pa.schema(
    [
        ("channel", pa.uint8()),
        ("x", pa.uint32()),
        ("y", pa.uint32()),
        ("z", pa.uint32()),
        ("color", pa.struct(color_struct_fields)),
    ]
)

This means that each row has 5 columns - the original channel (=PMT) of the
data, its coordinates in the array and its color as an RGB triplet. Since each
channel is rendered in grayscale, the value of the three color fields is
identical, so we can simply use one of them as the point's color. This schema
isn't required to read the data (as you'll see below), but it's useful to know
about it regardless.

The script uses a single stream as an example, and it also assumes we know the
"true" shape of the rendered data, (512, 512, 15) in our case. This shape is
determined by the configuration file used to generate the .arrow_stream file.
The output is a sparse matrix that can be post-processed using all kinds of
standard computational methods (averaging over all planes, e.g.), and can be
also written to disk in one format or another. It can also be visualized in
napari as a point cloud.
"""
import pathlib

import pyarrow as pa
import pyarrow.compute as pc
import sparse

filename = pathlib.Path(
    r"E:\Lior\2021_08_31\mouse3_crystal_fov3_2mag_750um_186kHz_two_channels_3_planes_visible_neurons.arrow_stream"
)
dense_data_shape = (512, 512, 15)
assert filename.exists()

opts = pa.ipc.IpcWriteOptions(allow_64bit=True)
stream = pa.ipc.open_stream(filename)


# Simple helper function below
def create_coords_list(recordbatch, mask):
    """Transform our RecordBatch format to a list of coordinates and values"""
    coords = []
    for col in recordbatch.columns[1:-1]:
        coords.append(col.filter(mask))

    data = recordbatch[-1].filter(mask).field("r")  # use a single color channel
    return (coords, data)


# Iterate over the Batches in the Arrow stream
for batch in stream:
    channels = pc.unique(batch[0])
    mask_per_channel = []
    for ch in channels[:-1]:
        mask_per_channel.append(pc.equal(ch, batch[0]))

    channel_data = []
    for mask in mask_per_channel:
        coords, data = create_coords_list(batch, mask)
        channel_data.append(
            sparse.COO(
                coords, data, dense_data_shape, has_duplicates=False, sorted=False
            )
        )

    for channel_idx, channel_datum in enumerate(channel_data):
        # Do something with the single channel data, such as write it to disk:
        # channel_datum.todense()  # numpy array with the original shape,
        # although many operations are available on the sparse representation
        # of the data
        pass

    break  # this loop should continue until exhausting the stream
