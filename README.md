# rPySight

Realtime rendering of photon streams in 2D and 3D arriving from a TimeTagger.

Uses a photon-counting based approach to improve the imaging conditions of a 2P microscope, but in contrast to [PySight](https://github.com/PBLab/python-pysight) the data is displayed in real time, which improves the user experience significantly.

## Project Structure

This project is a mixed Rust-Python project built using [maturin](https://github.com/PyO3/maturin). The core functionality (and most of the code base) is entirely in Rust.

## Requirements

A TimeTagger, Rust, Python and a 2P microscope.
