"""Convert a .ttbin file to a pickle file for PySight to parse.

rPySight produces ttbin files - i.e. native TT binary files - which need to
first be converted to pickle files if we wish to visualize them in PySight.
This tool parses these files according to the supplied configuration file
and produces a pickle file in the format PySight expects.

Usage:
    ttbin_to_pysight_pickle.py [-ho <output pickle filename] <input ttbin> <input configuration file>

-h --help   Show this message
-o <output pickle filename>  Write the output file to a specific location. By default the file will have the same name as the input file.
"""
from typing import Optional
from pathlib import Path

import numpy as np
from TimeTagger import FileReader
from docopt import docopt
import toml


EVENTS_PER_BATCH = 1_000_000


def _convert_and_validate(fname: str) -> Path:
    """Converts a user input string to a file object and validates it exists
    in the file system.

    Raises an AssertionError if it's not a valid path.
    """
    fname = Path(fname)
    assert fname.exists()
    return fname


def convert_ttbin(input_fname: str, output_fname: Optional[str], config_fname: str):

    input_fname = _convert_and_validate(input_fname)
    config_fname = _convert_and_validate(config_fname)
    if output_fname:
        output_fname = _convert_and_validate(output_fname)
    else:
        output_fname = input_fname.with_suffix('.pickle')
    config = toml.load(config_fname)

    loop_over_ttbins(input_fname, config, output_fname)


def get_overflow_indices(event_type: np.ndarray) -> np.ndarray:
    overflow_indices_start = np.where(event_type == 2)[0]
    total_length = len(event_type) 
    starts_len = len(overflow_indices_start)
    if starts_len > 0:
        overflow_indices_end = np.where(event_type == 3)[0]
        ends_len = len(overflow_indices_end)
        if ends_len == starts_len:
            return even_overflows(overflow_indices_start, overflow_indices_end, total_length)
        elif ends_len < starts_len:
            # We have a missed events situation between reads
            assert ends_len + 1 == starts_len
            less_end_overflows(overflow_indices_start, overflow_indices_end, total_length)
        else:
            assert ends_len == starts_len + 1
            more_end_overflows(overflow_indices_start, overflow_indices_end, total_length)
    else:
        return np.full_like(event_type, False)


def _iter_over_start_end_pairs(starts: np.ndarray, ends: np.ndarray, overflows: np.ndarray):
    for st, en in zip(starts, ends):
        overflows[st:en] = True
    return overflows


def even_overflows(starts: np.ndarray, ends: np.ndarray, length: int) -> np.ndarray:
    """Deals with the case where the number of starts and ends is identical"""
    overflows = np.full(length, False)
    overflows = _iter_over_start_end_pairs(starts, ends, overflows)
    return overflows


def less_end_overflows(starts: np.ndarray, ends: np.ndarray, length: int) -> np.ndarray:
    """Deals with the case where the overflow went on by to the next batch"""
    overflows = np.full(length, False)
    overflows = _iter_over_start_end_pairs(starts, ends, overflows)
    overflows[starts[-1]:] = True
    return overflows

    
def more_end_overflows(starts: np.ndarray, ends: np.ndarray, length: int) -> np.ndarray:
    """Deals with the case where the overflow continues from the previous batch"""
    overflows = np.full(length, False)
    overflows = _iter_over_start_end_pairs(starts, ends[1:], overflows)
    overflows[:ends[0]] = True
    return overflows


def loop_over_ttbins(input_fname, config, output_fname):
    reader = FileReader(input_fname)
    while reader.hasData():
        data = reader.getData(EVENTS_PER_BATCH)
        timestamps = data.getTimestamps().astype(np.uint64)
        channels = data.getChannels().astype(np.uint64)
        event_type = data.getEventTypes()
        overflow_indices = get_overflow_indices(event_type)
        timestamps = timestamps[~overflow_indices]
        channels = channels[~overflow_indices]

        
        





if __name__ == '__main__':
    args = docopt(__doc__)
    print(args)
    convert_ttbin(*args)
