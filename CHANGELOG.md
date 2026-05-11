# rPySight's Changelog

## Unreleased
* `build.rs` now provides a `main()` on Linux and Windows (previously macOS-only, broke builds on the other targets).
* `README.md` documents the required `--no-default-features` build flag for the `cli`/`gui` binaries and per-OS (Windows/macOS/Linux) build and runtime notes.

## Unreleased — claustrum overlay
* `.cargo/config.toml` carries an `x86_64-unknown-linux-gnu` rustflag pointing the linker at the GCC-11 libstdc++ directory, so claustrum (Ubuntu 22.04, GCC 12 default + only libstdc++-11-dev installed) links cleanly. No-op on Linux hosts where `libstdc++.so` is already on the default GCC's path.
* `README.md` adds a "Building on claustrum" subsection covering the conda-Python `LD_LIBRARY_PATH` hint and the headless-renderer caveat.

## 0.2.1 (Oct. 2021)
* Improved and fixed volumetric rendering.
* Upgraded to 2021 edition.
* Small fixes to accompanying Python scripts.

## 0.2.0 (Oct. 2021)
* Improvements to the arrow stream serialization format.
* Improvements to the helper scripts reading the arrow stream.
* Fixes a few 2D rendering issues.
* **Line and frame signals aren't support in the same acquisition - please choose one (you may record both and ignore one in the configuration file until this issue is resolved).**
* Performance upgrades.
* Configuration updates.
* Added the option to demultiplex two or more streams.


## 0.1.0 (Sep. 2021)
* Initial release.