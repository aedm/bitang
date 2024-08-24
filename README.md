# Bitang

A framework for creative coding, mainly targeted at demoscene productions. Currently in a very early stage of development.

![screenshot2](screenshot2.png)


## Features

- Text-based project description of all scenes, use your favorite text editor
- Live reload of all resources including shaders
- Shader variable extraction from GLSL code, just define your uniforms in the shader and they will appear on the UI

Not a lot of docs yet. Use the example project [`"Attractors"`](https://github.com/aedm/bitang/releases/tag/0.1.0) available on the releases page.


## Development

Dependencies:
- Rust
- Cmake, Ninja and Python (see https://github.com/aedm/bitang/issues/227)


### Build & run

Just run `cargo run --release`.

The development platform is Windows, but Linux and MacOS should work as well.


## Hardware requirements

Anything that supports Vulkan.


## License

MIT


![screenshot](screenshot.png)
