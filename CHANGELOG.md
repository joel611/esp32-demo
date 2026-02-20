# Changelog

## Unreleased

### Fixed

- **bindgen 0.64.0 arm64/aarch64 assertion failure on Apple Silicon** — `lvgl` uses `lvgl-sys` as a build-dependency (host build), where clang defaults to reporting `arm64-apple-darwin` while the `TARGET` env var is `aarch64-apple-darwin`. bindgen treats `arm64` as 32-bit (pointer size 4) and `aarch64` as 64-bit (pointer size 8), causing an assertion failure. Fixed by setting `BINDGEN_EXTRA_CLANG_ARGS_aarch64_apple_darwin = "--target=aarch64-apple-darwin"` in `.cargo/config.toml` — the target-specific variant is checked first by bindgen, so it only applies to host builds and does not interfere with ESP32 cross-compilation.

- **Missing ESP-IDF v5.3.3 submodules** — initialized missing submodules (esp-mqtt, mbedtls, etc.) in the embuild-managed ESP-IDF directory via `git submodule update --init --recursive`.

- **Rustup xtensa toolchain version mismatch** — the rustup xtensa toolchain (`esp-15.2.0_20250920`) is newer than what ESP-IDF v5.3.3 expects (`esp-13.2.0_20240530`). CMake finds the rustup binary regardless of PATH order and the `tool_version_check.cmake` raises a `FATAL_ERROR`. Since `esp-15.2.0` is functionally compatible with v5.3.3 code, fixed by setting `IDF_MAINTAINER = "1"` in `.cargo/config.toml` — a flag explicitly supported by the ESP-IDF cmake script to downgrade the mismatch to a warning.

- **LVGL build configuration** — added required env vars to `.cargo/config.toml`: `DEP_LV_CONFIG_PATH`, `CROSS_COMPILE`, `CFLAGS_xtensa_esp32s3_espidf` (`-mlongcalls`), `C_INCLUDE_PATH`, and `TARGET_C_INCLUDE_PATH` pointing to the xtensa-esp-elf toolchain headers.
