import polars as pl
import pyarrow as pa

df = pl.DataFrame({'a': [1, 2, 3], 'b': [None, 4, 5]})
df2 = pl.DataFrame({'a': [1, 9, 3], 'b': [None, 4, 5]})
print(df)

with open('target/d.dd', 'wb') as f:
    df.to_ipc(f)
    # df2.to_ipc(f)
    df2.to_ipc(f)

