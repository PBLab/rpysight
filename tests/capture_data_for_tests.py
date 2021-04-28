"""
A simple script which records data from the time tagger in a known fashion, to
be used as testing data.

It can also serve as an example for a data recording script for the timetagger.

The script should be run from the base directory of this project.
"""
from time import sleep
from datetime import datetime
import pathlib

from TimeTagger import createTimeTagger, FileWriter, FileReader
import numpy as np
import pandas as pd

# Create a timetagger instance
tagger = createTimeTagger()
tagger.reset()

# Acquisition parameters
data_directory = pathlib.Path('tests/data/').resolve()
recording_length = 200  # seconds
num_lines = 256
acceptable_laser_event_rate = 10  # MCPS
laser_division_factor = int(80 / acceptable_laser_event_rate)
extra_factors = '_x1_no_tag'
filename = data_directory / f'2d_test_data_{num_lines}_{recording_length}s{extra_factors}.ttbin'
print(f"Writing data to: {filename}")

# TimeTagger channels
LINE_START_CH = 1 # Rising edge on input 1
TAG_SYNC_CH = 2  # Rising edge on input 2
TA1000_CH = -3 # Falling edge on TA1000 PMT channel fed to input 3
TD2000_CH = -8 # Falling edge on TD2000 PMT channel fed to input 8
LASER_CH = 6 # Rising edge on input 6
channels = [LINE_START_CH, TAG_SYNC_CH, TA1000_CH, TD2000_CH, LASER_CH]

# Thresholds [V]
line_thresh = 1.2
tag_thresh = 2.0
ta1000_thresh = -0.3
td2000_thresh = -0.4
laser_thresh = 2.0

tagger.setTriggerLevel(LINE_START_CH, line_thresh)
tagger.setTriggerLevel(TAG_SYNC_CH, tag_thresh)
tagger.setTriggerLevel(TA1000_CH, ta1000_thresh)
tagger.setTriggerLevel(TD2000_CH, td2000_thresh)
tagger.setTriggerLevel(LASER_CH, laser_thresh)
tagger.setEventDivider(LASER_CH, laser_division_factor)

print("Starting the recording...")

file_writer = FileWriter(tagger, str(filename), channels)
sleep(recording_length)
file_writer.stop()
del file_writer

print("Acquisition done, let's see what we captured")

fr = FileReader(str(filename))
data = fr.getData(10000)
print(data.size)
