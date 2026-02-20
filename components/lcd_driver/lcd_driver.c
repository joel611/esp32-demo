#include "lcd_driver.h"

#include "freertos/FreeRTOS.h"
#include "freertos/task.h"
#include "freertos/semphr.h"
#include "driver/gpio.h"
#include "driver/spi_master.h"
#include "esp_rom_sys.h"
#include "esp_lcd_panel_io.h"
#include "esp_lcd_panel_ops.h"
#include "esp_log.h"
#include "esp_check.h"
#include "esp_lcd_sh8601.h"

static const char *TAG = "lcd_driver";

// ── Board pin definitions ────────────────────────────────────────────────────
#define LCD_H_RES         466
#define LCD_V_RES         466
#define LCD_BIT_PER_PIXEL  16   // RGB565

#define LCD_HOST      SPI2_HOST
#define PIN_LCD_CS     9
#define PIN_LCD_PCLK  10
#define PIN_LCD_D0    11
#define PIN_LCD_D1    12
#define PIN_LCD_D2    13
#define PIN_LCD_D3    14
#define PIN_LCD_RST    21
#define PIN_LCD_PWREN  42   // Display power enable (active HIGH)

// ── Init command tables ──────────────────────────────────────────────────────
// Set to 1 for CO5300 (most 2024+ hardware), 0 for SH8601.
#define USE_CO5300 1

// SH8601 init sequence
static const sh8601_lcd_init_cmd_t sh8601_init_cmds[] = {
    {0x11, NULL,                   0, 120},  // Sleep Out
    {0x44, (uint8_t[]){0x01,0xD1}, 2,   0},  // Set tear scanline
    {0x35, (uint8_t[]){0x00},      1,   0},  // TE On
    {0x53, (uint8_t[]){0x20},      1,  10},  // Write CTRL Display
    {0x51, (uint8_t[]){0x00},      1,  10},  // Brightness = 0
    {0x29, NULL,                   0,  10},  // Display On
    {0x51, (uint8_t[]){0xFF},      1,   0},  // Brightness = max
};

// CO5300 init sequence (requires x_gap=6)
static const sh8601_lcd_init_cmd_t co5300_init_cmds[] = {
    {0x11, NULL,              0,  80},  // Sleep Out
    {0xC4, (uint8_t[]){0x80}, 1,   0},  // Enable QSPI interface
    {0x53, (uint8_t[]){0x20}, 1,   1},  // Write CTRL Display
    {0x63, (uint8_t[]){0xFF}, 1,   1},  // HBM brightness
    {0x51, (uint8_t[]){0x00}, 1,   1},  // Brightness = 0
    {0x29, NULL,              0,  10},  // Display On
    {0x51, (uint8_t[]){0xFF}, 1,   0},  // Brightness = max
};

// ── Software SPI (replicates read_lcd_id_bsp) ───────────────────────────────
// Both working C examples (07, 09) call read_lcd_id() before power enable.
// This bit-bangs a SPI READ command which appears necessary to bring the
// display IC out of its power-on state before hardware QSPI takes over.

#define BIT_MASK (uint64_t)1

static void lcd_all_gpio_init(void)
{
    gpio_config_t cfg = {
        .mode         = GPIO_MODE_OUTPUT,
        .pull_up_en   = GPIO_PULLUP_ENABLE,
        .pull_down_en = GPIO_PULLDOWN_DISABLE,
        .intr_type    = GPIO_INTR_DISABLE,
        .pin_bit_mask = (BIT_MASK << PIN_LCD_CS)   |
                        (BIT_MASK << PIN_LCD_PCLK)  |
                        (BIT_MASK << PIN_LCD_D0)    |
                        (BIT_MASK << PIN_LCD_D1)    |
                        (BIT_MASK << PIN_LCD_D2)    |
                        (BIT_MASK << PIN_LCD_D3)    |
                        (BIT_MASK << PIN_LCD_RST),
    };
    gpio_config(&cfg);
}

static void d0_input_mode(void)
{
    gpio_config_t cfg = {
        .mode         = GPIO_MODE_INPUT,
        .pull_up_en   = GPIO_PULLUP_ENABLE,
        .pull_down_en = GPIO_PULLDOWN_DISABLE,
        .intr_type    = GPIO_INTR_DISABLE,
        .pin_bit_mask = BIT_MASK << PIN_LCD_D0,
    };
    gpio_config(&cfg);
}

static void d0_output_mode(void)
{
    gpio_config_t cfg = {
        .mode         = GPIO_MODE_OUTPUT,
        .pull_up_en   = GPIO_PULLUP_ENABLE,
        .pull_down_en = GPIO_PULLDOWN_DISABLE,
        .intr_type    = GPIO_INTR_DISABLE,
        .pin_bit_mask = BIT_MASK << PIN_LCD_D0,
    };
    gpio_config(&cfg);
}

static void spi_send_byte(uint8_t dat)
{
    for (int i = 0; i < 8; i++) {
        gpio_set_level(PIN_LCD_D0, (dat & 0x80) ? 1 : 0);
        dat <<= 1;
        gpio_set_level(PIN_LCD_PCLK, 0);
        gpio_set_level(PIN_LCD_PCLK, 1);
    }
}

static uint8_t spi_read_byte(void)
{
    uint8_t dat = 0;
    for (int i = 0; i < 8; i++) {
        gpio_set_level(PIN_LCD_PCLK, 0);
        d0_input_mode();
        esp_rom_delay_us(1);
        dat = (dat << 1) | gpio_get_level(PIN_LCD_D0);
        d0_output_mode();
        gpio_set_level(PIN_LCD_PCLK, 1);
        esp_rom_delay_us(1);
    }
    return dat;
}

// Replicates read_lcd_id() from read_lcd_id_bsp.c.
// Configures all SPI pins as GPIO, performs hardware reset, then sends a
// software-SPI read of register 0xDA (RDID1) to detect the controller.
// CS is held LOW (from lcd_all_gpio_init latch=0) throughout — this matches
// exactly what the reference examples do.
static uint8_t soft_spi_read_lcd_id(void)
{
    lcd_all_gpio_init();   // all pins → GPIO output (latch=0 → CS,CLK,D0-3 LOW)

    // Hardware reset sequence (RST driven explicitly)
    gpio_set_level(PIN_LCD_RST, 1);
    vTaskDelay(pdMS_TO_TICKS(120));
    gpio_set_level(PIN_LCD_RST, 0);
    vTaskDelay(pdMS_TO_TICKS(120));
    gpio_set_level(PIN_LCD_RST, 1);
    vTaskDelay(pdMS_TO_TICKS(120));

    // Send read command for RDID1 (CS stays LOW from lcd_all_gpio_init)
    spi_send_byte(0x03);  // read opcode
    spi_send_byte(0x00);
    spi_send_byte(0xDA);  // RDID1 register
    spi_send_byte(0x00);  // PAM

    uint8_t id = spi_read_byte();
    ESP_LOGI(TAG, "LCD ID: 0x%02x", id);
    return id;
}

// ── State ────────────────────────────────────────────────────────────────────
static esp_lcd_panel_handle_t s_panel = NULL;
static SemaphoreHandle_t s_flush_sem  = NULL;

// Called from SPI ISR when the pixel DMA transfer finishes.
static bool on_color_trans_done(esp_lcd_panel_io_handle_t panel_io,
                                esp_lcd_panel_io_event_data_t *edata,
                                void *user_ctx)
{
    BaseType_t high_task_awoken = pdFALSE;
    xSemaphoreGiveFromISR(s_flush_sem, &high_task_awoken);
    return high_task_awoken == pdTRUE;
}

// ── Public API ───────────────────────────────────────────────────────────────

esp_err_t lcd_driver_init(void)
{
    s_flush_sem = xSemaphoreCreateBinary();
    ESP_RETURN_ON_FALSE(s_flush_sem, ESP_ERR_NO_MEM, TAG, "flush semaphore alloc failed");

    // ── Software SPI ID read + hardware reset (matches reference examples) ────
    uint8_t lcd_id = soft_spi_read_lcd_id();

    // ── Display power enable (GPIO 42, active HIGH) ───────────────────────────
    gpio_config_t pwr_cfg = {
        .mode = GPIO_MODE_OUTPUT,
        .pin_bit_mask = 1ULL << PIN_LCD_PWREN,
        .pull_up_en = GPIO_PULLUP_ENABLE,
    };
    gpio_config(&pwr_cfg);
    gpio_set_level(PIN_LCD_PWREN, 1);
    vTaskDelay(pdMS_TO_TICKS(10));

    // SPI QSPI bus
    const spi_bus_config_t bus_cfg =
        SH8601_PANEL_BUS_QSPI_CONFIG(PIN_LCD_PCLK,
                                     PIN_LCD_D0, PIN_LCD_D1,
                                     PIN_LCD_D2, PIN_LCD_D3,
                                     LCD_H_RES * LCD_V_RES * LCD_BIT_PER_PIXEL / 8);
    ESP_RETURN_ON_ERROR(spi_bus_initialize(LCD_HOST, &bus_cfg, SPI_DMA_CH_AUTO),
                        TAG, "SPI bus init failed");

    // Panel IO
    esp_lcd_panel_io_handle_t io = NULL;
    const esp_lcd_panel_io_spi_config_t io_cfg =
        SH8601_PANEL_IO_QSPI_CONFIG(PIN_LCD_CS, on_color_trans_done, NULL);
    ESP_RETURN_ON_ERROR(
        esp_lcd_new_panel_io_spi((esp_lcd_spi_bus_handle_t)LCD_HOST, &io_cfg, &io),
        TAG, "panel IO init failed");

    // Panel driver – select init table by detected ID (0x86=SH8601, else CO5300)
    bool is_sh8601 = (lcd_id == 0x86);
    ESP_LOGI(TAG, "Using %s init sequence", is_sh8601 ? "SH8601" : "CO5300");
    sh8601_vendor_config_t vendor_cfg = {
        .init_cmds      = is_sh8601 ? sh8601_init_cmds : co5300_init_cmds,
        .init_cmds_size = is_sh8601
            ? sizeof(sh8601_init_cmds) / sizeof(sh8601_init_cmds[0])
            : sizeof(co5300_init_cmds) / sizeof(co5300_init_cmds[0]),
        .flags = { .use_qspi_interface = 1 },
    };

    const esp_lcd_panel_dev_config_t panel_cfg = {
        .reset_gpio_num = PIN_LCD_RST,
        .rgb_ele_order  = LCD_RGB_ELEMENT_ORDER_RGB,
        .bits_per_pixel = LCD_BIT_PER_PIXEL,
        .vendor_config  = &vendor_cfg,
    };
    ESP_RETURN_ON_ERROR(esp_lcd_new_panel_sh8601(io, &panel_cfg, &s_panel),
                        TAG, "panel create failed");

    ESP_RETURN_ON_ERROR(esp_lcd_panel_reset(s_panel),       TAG, "panel reset failed");
    ESP_RETURN_ON_ERROR(esp_lcd_panel_init(s_panel),        TAG, "panel init failed");
    ESP_RETURN_ON_ERROR(esp_lcd_panel_disp_on_off(s_panel, true), TAG, "display on failed");

    if (!is_sh8601) {
        // CO5300 has a 6-pixel horizontal offset
        esp_lcd_panel_set_gap(s_panel, 6, 0);
    }

    ESP_LOGI(TAG, "LCD ready: %d x %d, RGB565", LCD_H_RES, LCD_V_RES);
    return ESP_OK;
}

void lcd_draw_bitmap(int x1, int y1, int x2, int y2, const void *data)
{
    // esp_lcd_panel_draw_bitmap enqueues CASET+RASET (sync) then the
    // pixel DMA (async).  We block on s_flush_sem until the ISR callback
    // signals that the DMA is complete.
    esp_lcd_panel_draw_bitmap(s_panel, x1, y1, x2, y2, data);
    xSemaphoreTake(s_flush_sem, portMAX_DELAY);
}
