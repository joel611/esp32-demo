## Common Commands
```
cargo build

cargo espflash flash --monitor
```

## Project Structure
- `components/lcd_driver/` — C LCD driver (SH8601/CO5300 QSPI AMOLED, 466×466)
- `components/esp_lcd_sh8601/` — local copy of esp-idf SH8601 panel driver
- `lvgl-configs/lv_conf.h` — active LVGL config (referenced via DEP_LV_CONFIG_PATH)
- `sdkconfig.defaults` — FreeRTOS/stack config

## Hardware Gotchas
- Display init order in lcd_driver_init(): soft_spi_read_lcd_id() → GPIO 42 HIGH → hardware SPI
  - soft SPI bit-bangs a register read BEFORE hardware SPI; without it display stays blank
  - GPIO 42 is display power enable (active HIGH); without it display stays blank
- LCD ID 0xFF = CO5300, 0x86 = SH8601 (init sequences differ; CO5300 needs x_gap=6)

## LVGL Setup Rules
- LV_COLOR_16_SWAP 1 required (ESP32 little-endian byte order)
- LVGL 8.x stores a pointer to lv_disp_drv_t/lv_disp_draw_buf_t — both must be Box::leak()ed
- lv_area_t coords are inclusive; esp_lcd_panel_draw_bitmap expects exclusive x2/y2 (add 1)
- Stack: CONFIG_ESP_MAIN_TASK_STACK_SIZE=32768 in sdkconfig.defaults

## Debugging Tips
- C component changes not taking effect? CMake cache is stale — check serial log for missing
  expected messages or wrong timing. Fix: rm -rf target/xtensa-esp32s3-espidf/debug/build/esp-idf-sys-*/
- cargo espflash flash --monitor exits early in non-interactive terminals; flash succeeds, monitor fails

## Git Worktrees
- Worktree dir: `.worktree/` (singular). Each worktree has its OWN copy of `components/` — changes
  to the main branch's `components/` are NOT visible to the worktree. Edit the worktree copy directly.
- After modifying C components in a worktree, the CMake library may be stale even after clearing
  esp-idf-sys-*/. Force full rebuild: rm -rf target/xtensa-esp32s3-espidf/debug/build/esp-idf-sys-<hash>/

## LVGL Draw Buffer Limits
- esp-lcd SPI driver calls esp_ptr_dma_capable() — PSRAM buffers are always rejected at transmit time
  even with MALLOC_CAP_DMA flag. Max usable DMA buffer: ~100 rows × 466px × 2B ≈ 91KB.
- Double-buffer async DMA pattern: lcd_wait_flush_done() → lcd_draw_bitmap_async() → lv_disp_flush_ready()
  LVGL renders into one buffer while the other DMA's to the display.
