import os
import pathlib
from TimeTagger import FileReader
import numpy as np
import pandas as pd
import pickle
import glob

print("")
data_directory = r'E:\Lior' + os.sep + '2021_08_31' + os.sep

file_wile_card = data_directory + '*[a-z].ttbin'
discarded_file_wile_card = data_directory + '*[0-9].ttbin' # remove the list arising from this wild card from the list above

unfiltered_fn_list = glob.glob(file_wile_card)
discarded_fn_list = glob.glob(discarded_file_wile_card)

keep_this_fn_list = list(set(unfiltered_fn_list)-set(discarded_fn_list)) # using sets to quickly remove the unneeded values, as per https://stackoverflow.com/questions/2514961/remove-all-values-within-one-list-from-another-list/30353802 

print(str(len(keep_this_fn_list)) + ' files found')

#unique_channels = np.unique(sanitized_channels, return_index=True)

LINE_START_CH = 1 # Rising edge on input 1
TAG_SYNC_CH = 5  # Rising edge on input 2
TA1000_CH = 8 # Falling edge on TA1000 PMT channel fed to input 3
TD2000_CH = -6 # Falling edge on TD2000 PMT channel fed to input 8
LASER_CH = 3  # Rising edge on input 6
TREADMILL_CH = 2 # Rising edge on input 5

LINE_END_CH = -LINE_START_CH
TAG_END_CH = -TAG_SYNC_CH  
TREADMILL_END_CH = -TREADMILL_CH

n_events = 1_000_000_000  # Number of events to read at once

channel_tt_value_list = [LINE_START_CH, TAG_SYNC_CH, TA1000_CH, TD2000_CH, LASER_CH, TREADMILL_CH, LINE_END_CH, TAG_END_CH, TREADMILL_END_CH] # [1, 2, -3, -8, 6]

channel_identity = ["Lines", "TAG Lens", "PMT1", "PMT2", "Laser", "Treadmill", "Line Ends", "TAG Ends", "Treadmill Ends"]

for fnum, fname in enumerate(keep_this_fn_list):

    print("1: Read the data from " + fname)

    file_reader = FileReader(fname)
    # now the data can be accessed via methods of the file_reader object
    pickle_file_counter = 0 # split each ttbin acquisition into several pickle files, each n_events long


    # now the data can be accessed via methods of the file_reader object
    total_event_counter = 0
    missed_event_counter = 0
    

    while file_reader.hasData():

        data = file_reader.getData(n_events)
        channel = data.getChannels()
        timestamps = data.getTimestamps()
        # TimeTag = 0, Error = 1, OverflowBegin = 2, OverflowEnd = 3, MissedEvents = 4
        overflow_types = data.getEventTypes()

        print(f'number of invalid events is: {np.sum(overflow_types)}')
        # dump all events else than legitimate time tags:

        sanitized_channels = channel[overflow_types == 0]

        sanitized_timestamps = timestamps[overflow_types == 0].astype('uint64')
        smack_my_dict_up = {}

        for channel_index, channel_tt_value in enumerate(channel_tt_value_list):

            smack_my_dict_up[channel_identity[channel_index]] = pd.DataFrame({"abs_time": 

                                        sanitized_timestamps[sanitized_channels == channel_tt_value]})

        pickle_fn = fname.replace('.ttbin',  '_' + str(pickle_file_counter) + '.p')
        print('saving to ' + pickle_fn)

        with open(pickle_fn, 'wb') as f:
            pickle.dump(smack_my_dict_up, f, protocol=4)

        if pickle_file_counter == 0:
            line_diff = smack_my_dict_up["Lines"].diff()
            print(f'line_diff is:')
            print(line_diff)

            print(f'TAG diff is: {smack_my_dict_up["TAG Lens"].diff()}')

            print(f'PMT1 events are:')
            print(smack_my_dict_up["PMT1"])

            print(f'PMT2 events are:')
            print(smack_my_dict_up["PMT2"])

            print(f'Laser diff is: {smack_my_dict_up["Laser"].diff()}')

            print(f'Treadmill events are:')
            print(smack_my_dict_up["Treadmill"])

        pickle_file_counter += 1 # increment pickle file number for the same ttbin file

