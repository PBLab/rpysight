# RPySight

Real time parsing of TimeTagger data streams to display live images and volumes

## Project Structure

This project is a mixed Rust-Python project built using [maturin](https://github.com/PyO3/maturin). The full package is published to PyPI but the core functionality (and most of the code base) is entirely in Rust.

Even though this project is published to PyPI (eventually), the main configuration file is `Cargo.toml` that contains most of the metadata for both the Python and Rust bits of this work.  

https://play.rust-lang.org/?version=nightly&mode=release&edition=2018&gist=70482ff3b2f6a0923452daca083df220
