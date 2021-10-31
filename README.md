# Egui FLTK Frontend

[![Crates.io](https://img.shields.io/crates/v/egui-fltk-frontend.svg)](https://crates.io/crates/egui-fltk-frontend)
[![Documentation](https://docs.rs/egui-fltk-frontend/badge.svg)](https://docs.rs/egui-fltk-frontend)
[![CI](https://github.com/Ar37-rs/egui-fltk-frontend/actions/workflows/ci.yml/badge.svg)](https://github.com/Ar37-rs/egui-fltk-frontend/actions/workflows/ci.yml)

[FLTK](https://github.com/fltk-rs/fltk-rs) Frontend for [Egui WGPU Backend](https://github.com/hasenbanck/egui_wgpu_backend)

On linux Debian/Ubuntu, make sure to install the latest main requirements:

```
sudo apt-get update && sudo apt-get install -y libpango1.0-dev libx11-dev libxext-dev libxft-dev libxinerama-dev libxcursor-dev libxrender-dev libxfixes-dev libgl1-mesa-dev libglu1-mesa-dev libmpv-dev
```

Additional linux requirements:

```
sudo apt-get install libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libspeechd-dev libxkbcommon-dev libssl-dev
```

## Example

Running examples/main.rs file:

```
cargo run --example main
```

To use clipboard functionality on X11 system run following command to avoid pulling unnecessary wayland dependencies:

```
cargo run --example main --no-default-features --features=clipboard-x11
```

or [click here](https://github.com/Ar37-rs/egui-fltk-frontend/tree/main/examples) on how to use it inside Cargo.toml
