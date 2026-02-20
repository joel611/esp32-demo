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
 *
 * @param x1     Left column (inclusive)
 * @param y1     Top row (inclusive)
 * @param x2     Right column (exclusive, i.e. x2 = area.x2 + 1)
 * @param y2     Bottom row (exclusive)
 * @param data   RGB565 pixel buffer
 */
void lcd_draw_bitmap(int x1, int y1, int x2, int y2, const void *data);

#ifdef __cplusplus
}
#endif
