idf_component_register(
    SRCS "lcd_driver.c"
    INCLUDE_DIRS "include"
    REQUIRES esp_lcd driver freertos esp_timer esp_lcd_sh8601
)
