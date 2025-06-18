# Audio Virtualizer

A real-time surround sound to stereo virtualizer that uses Head-Related Transfer Function (HRTF) convolution to create immersive 3D audio for headphones.
Supports only standard 7.1-channel layout: FL, FR, FC, LFE, BL, BR, SL, SR.

**Currently supports macOS only.**

## Prerequisites

- BlackHole 16ch driver (https://existential.audio/blackhole)

## Building

Install cargo-bundle:
```shell
cargo install cargo-bundle
```

Build a macOS app bundle:
``` shell
cargo bundle --release
```