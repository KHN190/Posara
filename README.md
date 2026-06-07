<p align="center"><img src="./assets/banner.svg" width="720"></p>

Your creative visualaudio toolkit for MacOS 1-bit aesthetics.

- **Screen** — minifb
- **Controller** — 8-button bitmap + last ASCII key
- **Audio** — 4 channels (sample + pitch + stereo volume + ADSR)
- **MIDI** — note in / out

Run on MacOS / Linux / Windows. Bare metals TBA.

<p align="center"><img src="./assets/showcase.png" width="720"></p>

## Build & Run

```sh
cargo build --release
posara run [--root <dir>] [--profile] [--headless] <cart.abe | cart.pk>
```

The interpreter is watched and hot-reloaded.

## License

MIT
