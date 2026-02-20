# ESP32 Display

Rust firmware for an ESP32-S3 display project using LVGL and ESP-IDF.

## Prerequisites

### 1. Install Rust for ESP32

Install [espup](https://github.com/esp-rs/espup), which sets up the Xtensa Rust toolchain and cross-compiler:

```sh
cargo install espup
espup install
```

After installation, `espup` creates `~/export-esp.sh` which configures the required environment variables.

### 2. Install `ldproxy` and `espflash`

```sh
cargo install ldproxy
cargo install espflash
```

### 3. Shell Environment Setup

Before running `cargo build`, source the ESP environment script:

```sh
source ~/export-esp.sh
```

This adds the Xtensa cross-compiler (`xtensa-esp32s3-elf-gcc`) to your `PATH`, which is required for the LVGL C sources to compile.

**To avoid running this every session**, add it to your shell profile:

```sh
# Add to ~/.zshrc or ~/.bashrc
source ~/export-esp.sh
```

## Building

```sh
cargo build
```

## Flashing

```sh
cargo espflash flash --monitor
```

## Configuration Notes (`.cargo/config.toml`)

Several paths in `.cargo/config.toml` are machine-specific and may need updating:

| Variable | Description |
|---|---|
| `CC_xtensa_esp32s3_espidf` | Path to `xtensa-esp32s3-elf-gcc`. Run `cat ~/export-esp.sh` to find your toolchain path. |
| `LIBCLANG_PATH` | Path to `libclang.dylib` from the espup xtensa clang. |
| `TARGET_C_INCLUDE_PATH` | Path to xtensa C headers (from the espup toolchain). |
| `C_INCLUDE_PATH` | Same as above. |

To find the correct paths on your machine:

```sh
# Find your xtensa toolchain version and path
cat ~/export-esp.sh

# Example output:
# export PATH="/Users/<you>/.rustup/toolchains/esp/xtensa-esp-elf/esp-13.2.0_20240530/xtensa-esp-elf/bin:$PATH"
```

Update the paths in `.cargo/config.toml` accordingly.

## Troubleshooting

See `CHANGELOG.md` for a full list of build issues and their fixes, including:

- `bindgen` 0.64.0 arm64/aarch64 assertion failure on Apple Silicon
- ESP-IDF submodule initialization
- ESP-IDF version compatibility (use v5.3.3 with current crate versions)
- CMake toolchain version mismatch (`IDF_MAINTAINER`)
- `xtensa-esp32s3-elf-gcc` not found (missing `source ~/export-esp.sh`)
