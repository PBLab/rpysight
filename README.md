# rPySight

Realtime rendering of photon streams in 2D and 3D arriving from a TimeTagger.

## Project Structure

rPySight reads data from an incoming TimeTagger stream and displays it on screen. We interact with the TimeTagger from a Python script that is run inside the main Rust program. Thus, it's easier to build this project with [maturin](https://github.com/PyO3/maturin), even thought this is a Rust (Cargo) project.

## Requirements

A TimeTagger, Rust, Python and a two-photon microscope.
