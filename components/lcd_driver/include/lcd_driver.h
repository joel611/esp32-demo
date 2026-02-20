#pragma once

#include <stdbool.h>
#include "esp_err.h"

#ifdef __cplusplus
extern "C" {
#endif

/**
 * Initialize the QSPI LCD hardware.
 * Must be called before any drawing operations.
 */
esp_err_t lcd_driver_init(void);

/**
 * Flush pixel data to the display.
 * Blocks until the DMA transfer completes.
 */
void lcd_draw_bitmap(int x1, int y1, int x2, int y2, const void *data);

/**
 * Start a pixel DMA transfer and return immediately (non-blocking).
 * Call lcd_wait_flush_done() before the next draw or before touching the buffer.
 */
void lcd_draw_bitmap_async(int x1, int y1, int x2, int y2, const void *data);

/**
 * Block until the most recent lcd_draw_bitmap_async() transfer is complete.
 * No-op if no transfer is in flight.
 */
void lcd_wait_flush_done(void);

#ifdef __cplusplus
}
#endif
