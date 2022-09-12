# Egui FLTK Frontend

[![Crates.io](https://img.shields.io/crates/v/egui-fltk-frontend.svg)](https://crates.io/crates/egui-fltk-frontend)
![minimum rustc 1.61.0](https://img.shields.io/badge/rustc-1.61.0-blue.svg)
[![Documentation](https://docs.rs/egui-fltk-frontend/badge.svg)](https://docs.rs/egui-fltk-frontend)
[![CI](https://github.com/Ar37-rs/egui-fltk-frontend/actions/workflows/ci.yml/badge.svg)](https://github.com/Ar37-rs/egui-fltk-frontend/actions/workflows/ci.yml)

[FLTK](https://github.com/fltk-rs/fltk-rs) frontend for [egui](https://github.com/emilk/egui) [WGPU](https://github.com/gfx-rs/wgpu).

## On linux Debian/Ubuntu based distros, install latest build tools:

```
sudo apt-get update && sudo apt-get install build-essential cmake ninja-build
```

make sure to install the latest FLTK requirements:

```
sudo apt-get update && sudo apt-get install -y libpango1.0-dev libx11-dev libxext-dev libxft-dev libxinerama-dev libxcursor-dev libxrender-dev libxfixes-dev libgl1-mesa-dev libglu1-mesa-dev
```

and egui requirements as well:

```
sudo apt-get install libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libspeechd-dev libxkbcommon-dev libssl-dev
```

## Usage

```toml
[dependencies]
egui-fltk-frontend = "0.22.1"
```

Note:
on xwayland based desktop (like gnome 41+) doesn't require to enable the "wayland" features.

## Example

Running example *.rs files:

```
cargo r --example image
cargo r --example main
cargo r --example smaa
cargo r --example custom3d
```

or [click here](https://github.com/Ar37-rs/egui-fltk-frontend/tree/main/examples) on how to use it inside Cargo.toml

## Screenshot

[main_example](https://github.com/Ar37-rs/egui-fltk-frontend/tree/main/examples/main_example) running on WSL2 + X Server:

![alt_test](screenshot/main.png)
