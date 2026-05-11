# rPySight

Real-time rendering of photon streams in 2D and 3D arriving from a [TimeTagger](https://www.swabianinstruments.com/time-tagger/), full project published in [Har-Gil et al 2022 Neurphothonics 9(3): 039120](https://www.spiedigitallibrary.org/journals/neurophotonics/volume-9/issue-3/031920/Versatile-software-and-hardware-combo-enabling-photon-counting-acquisition-and/10.1117/1.NPh.9.3.031920.full)

## Motivation

Photon counting is an imaging approach where detected photons are discriminated before being digitized, eliminating a large source of error from typical brightness measurements in the live imaging world, especially using two-photon microscopes. While this approach ultimately provides better-looking images, it also suffers from a higher entry bar and a general lack of advocates in the (neuroscientific) imaging community.  However, once photon counting is fully implemented it can also help experimenters to introduce other advanced imaging modalities, such as volumetric imaging. 

## Introduction

rPySight aims to ameliorate some of the difficulties in implementing photon counting by providing researchers and users with a high quality application for the rendering part of the photon counting microscope. Together with proper hardware (the previously mentioned TimeTagger) implementing photon counting should be quite easy and within reach for most users, even the less tech-savvy ones. We, at the lab of [Dr. Pablo Blinder](http://pblab.tau.ac.il/en/), already provided a solution for these issues in the form of [PySight](https://github.com/PBLab/python-pysight), a Python package that achieves similar goals. However, PySight had one major deficit (besides its [sub-par lead contributor](https://github.com/PBLab/python-pysight/graphs/contributors)) - it did its magic offline, which added an exhausting post-processing step for experimenters, and also meant that you're never quite sure how did the imaging session go until you've analyzed the data.

rPySight's main _raison d'être_ is the fact that it shows the same data but in **real time.** This is possible due to a few technical upgrades and changes done at the hardware and software level, but the main benefit is clear - experimenters can again see their samples during the imaging session. Moreover, rPySight even does real-time 3D rendering of data captured with a [TAG lens](https://www.mitutoyo.com/taglens/). This novel feat is more than an incremental quality-of-life improvement, by allowing TAG lens users to have live feedback during their experiments, rather than having a mediocre Z-projected image to work with.

## A Few Technical Details and Requirements

This project is a mixed Rust-Python project - most of the work is done with Rust, but Python is required to start the TimeTagger and stream the data from it to the Rust renderer. You may use [maturin](https://github.com/PyO3/maturin)to more easily build the project locally. Thus a recent Rust compiler (when building from soure) and an updated Python version are needed to run this project. Needless to say, a working and installed TimeTagger is also required.

## Installation and Usage

A detailed protocol can be found in the accompanying manuscript (currently being written), or in the [tutorial file](https://github.com/PBLab/rpysight/blob/main/TUTORIAL.md) provided in this repo.

### Install from source (recommended)

Download a Rust compiler, preferably using [rustup](https://rustup.rs/), clone the repo, then build the binary with:

```
cargo build --release --bin cli --no-default-features
```

The `--no-default-features` flag is required for the standalone binaries on every OS. `Cargo.toml` ships `default = ["extension-module"]` so that the crate also builds as a Python extension via `maturin`; the `extension-module` PyO3 feature tells PyO3 *not* to link `libpython`, which is wrong for the `cli`/`gui` binaries (they *embed* Python via `auto-initialize` and need `libpython` linked). Disabling defaults restores the embed-style link.

Next, edit `rpysight/call_timetagger.py` and point the marked directories at your TimeTagger installation. To run:

```
cargo run --release --bin cli --no-default-features -- CONFIG_FILENAME
```

(the `--` separates cargo's args from the rPySight CLI's args; the config TOML follows). A GUI build is also available via `--bin gui` but is clunkier.

#### Per-OS build notes

**Windows (the TimeTagger workstation target).** Set PowerShell environment variables before building so PyO3 finds the right Python:

```
$Env:PYTHONHOME = "C:\Users\USERNAME\.conda\envs\timetagger\"
$Env:PYO3_PYTHON = "C:\Users\USERNAME\.conda\envs\timetagger\python.exe"
```

These same variables must be set in the shell that runs `cli.exe` — Windows finds `python3X.dll` via `PYTHONHOME`'s `DLLs` neighbours and the conda env's root. See `TUTORIAL.md` step 4 for the canonical example.

**macOS.** The bundled `.cargo/config.toml` already adds `-undefined dynamic_lookup` for both `x86_64-apple-darwin` and `aarch64-apple-darwin`. `build.rs` adds an rpath for the CommandLineTools framework directory. No extra flags should be needed beyond `--no-default-features`. Make sure `python3-config --prefix` resolves to the Python whose `libpython3X.dylib` you intend to embed (set `PYO3_PYTHON` to override).

**Linux.** Beyond `--no-default-features`, the runtime needs `libpython3X.so.1.0` discoverable at exec time. The typical incantation is `LD_LIBRARY_PATH=$(python3 -c 'import sysconfig; print(sysconfig.get_config_var("LIBDIR"))') ./target/release/cli config.toml`.

If `cc`/lld fails with `unable to find library -lstdc++`, the system is missing the unversioned `libstdc++.so` dev symlink for the active GCC — install `libstdc++-N-dev` matching the output of `gcc -dumpversion` (e.g. `apt install libstdc++-12-dev` for GCC 12).

Per-host workstation setups (e.g. specific conda paths, headless build flags) are kept on dedicated branches rather than here — see `git branch -a` for the host-specific overlays available.

### Download binary file

Download the binary from the Releases page and run it in your shell.

### Usage

Using rPySight is quite simple and can be boiled down to following these simple steps:

![Using rPySight](resources/rpysight_config_scheme.png)

More information can be found in the [tutorial](https://github.com/PBLab/rpysight/blob/main/TUTORIAL.md), or by contacting the authors of this work.
### Outputs

rPySight generates two main outputs with names similar to the ones in the "filename" field of the configuration file. The first is a `.ttbin` file that can be used to replay old experiments and generally have access to the raw data as it arrived from the TimeTagger. The second is an `.arrow_stream` file, which is a table of coordinates and data (i.e. a sparse matrix) that can be used to create the same rendered volumes but in post-processing. An example for such processing in Python may be found in the `rpysight` directory.
