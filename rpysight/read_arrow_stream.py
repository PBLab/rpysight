import pathlib

import pyarrow as pa

filename = pathlib.Path(r"E:\Lior\2021_08_31\mouse2_arterial_fov1_3mag_500um_186kHz.arrow")

assert filename.exists()

struct_fields = [
    ("x", pa.float32()),
    ("y", pa.float32()),
    ("z", pa.float32()),
]
schema = pa.schema([
    ("channels", pa.uint8()),
    ('x', pa.uint32()),
    ('y', pa.uint32()),
    ('z', pa.uint32()),
    ("colors", pa.struct(struct_fields))

])

opts = pa.ipc.IpcWriteOptions(allow_64bit=True)
stream = pa.ipc.open_stream(
    filename
)
print(stream.schema)
for b in stream:
    print(b)
    break


