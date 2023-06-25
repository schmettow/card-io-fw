Card/IO firmware
================

This repository contains the firmware source code for the Card/IO open source ECG device, built with
an ESP32-S3 MCU.

This firmware is in its early stages of development.

Setup
-----

Tools you need to build the firmware:

- Espressif's Xtensa-enabled rust compiler - [espup](https://github.com/esp-rs/espup)
  > Make sure to run `. ~/export-esp.sh` before trying to work with the firmware
- `cargo install cargo-espflash`
- `cargo install cargo-watch`

Commands
--------

- `cargo xtask -h`: Prints information about available commands. Most of the commands have short
  aliasses, listed below.
- `cargo xbuild <hw>`: Build the firmware for a `<hw>` version board.
- `cargo xrun <hw>`: Build and run the firmware on a `<hw>` version board.
  `<hw>` can be omitted, or one of: `v1`, `v2`. Defaults to the latest version.
- `cargo xcheck <hw>`: runs `cargo check`
- `cargo xclippy <hw>`: runs `cargo clippy`
- `cargo xdoc <hw> [--open]`: runs `cargo doc` and optionally opens the generated documentation.
- `cargo xtest`: runs `cargo test`.
- `cargo example <package> <example> [--watch]`: runs an example.
  Use `--watch` to enable automatic reload when a file changes.
- To run the config site on your PC, run `cargo watch -x "example config-site simple"`
  and open `127.0.0.1:8080` in a browser.
