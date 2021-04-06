from rpysight import call_timetagger


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
