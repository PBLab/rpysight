import numpy as np

simple_arange = np.atleast_2d(np.arange(10)).T
type_ = np.zeros((10, 1), dtype=np.uint16)
missed_events = np.ones((10, 1), dtype=np.uint8)
channel = np.ones((10, 1), dtype=np.int32) * 2
time = np.ones((10, 1), dtype=np.int64) * 3
