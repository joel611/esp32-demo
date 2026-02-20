
## HARDWARE SPEC
- Microcontroller: ESP32-S3R8
  - Dual-core Xtensa LX7 processor, 8MB PSRAM.
  - Built-in Bluetooth 5.0 and 2.4GHz Wi-Fi.
  - GPIO pins for SPI (display) and I2C (touch).
- Display: 466x466 AMOLED Touch Circular Display
  - High-resolution, vibrant display for clear visuals.
  - Capacitive touch for user interaction.
  - Driver:
    - Display (QSPI): SH8601,
    - Touch (I2C): FT3168
- Power Supply:
  - Rechargeable Li-Po battery (3.7V, 1000mAh) for portability.
  - USB-C port for charging and debugging.
- Chips:
  - PCF85063: RTC chip (I2C address: 0x51)
  - QMI8658c: IMU chip - reading and printing accelerometer data, gyroscope data, and temperature data (I2C address: 0x6B)

## Working Configuration

### I2C Configuration
- **Bus**: I2C0
- **Speed**: 600 kHz (working example uses 600 kHz, not 400 kHz)
- **SDA**: GPIO 47
- **SCL**: GPIO 48

### I2C Device Addresses (Confirmed)
| Device | Address | Description |
|--------|---------|-------------|
| FT3168 | 0x38    | Touch controller |
| PCF85063 | 0x51  | RTC chip |
| QMI8658c | 0x6B  | IMU (accelerometer/gyroscope) |

### SPI Configuration (Display)
- **Bus**: SPI2
- **Speed**: 80 MHz
- **Mode**: QSPI

## Pin Mapping

### AMOLED Display (QSPI)

| Signal       | ESP32-S3 Pin | Description           |
| ------------ | ------------ | --------------------- |
| QSPI_CS      | GPIO 9       | QSPI chip selection   |
| QSPI_CLK     | GPIO 10      | QSPI clock            |
| QSPI_D0      | GPIO 11      | QSPI data 0 (MOSI)    |
| QSPI_D1      | GPIO 12      | QSPI data 1           |
| QSPI_D2      | GPIO 13      | QSPI data 2           |
| QSPI_D3      | GPIO 14      | QSPI data 3           |
| AMOLED_RESET | GPIO 21      | Display reset (active low) |
| AMOLED_EN    | GPIO 42      | Display enable (active high) |

### Touch Controller (I2C)
| Signal  | ESP32-S3 Pin | Description     |
| ------- | ------------ | --------------- |
| TP_SDA  | GPIO 47      | I2C data line   |
| TP_SCL  | GPIO 48      | I2C clock line  |

## Initialization Sequence

### Display (SH8601)
1. Enable display (GPIO 42 HIGH)
2. Hardware reset (GPIO 21 LOW → delay 10ms → HIGH)
3. Wait 120ms
4. Send initialization commands via QSPI
5. Display should show colors correctly

### Touch Controller (FT3168)
1. Wait 200ms after I2C initialization
2. Write `0x00` to register `0x00` to enter normal mode
3. Touch coordinates available at register `0x03` (4 bytes)
4. Touch count available at register `0x02`

## Notes
- Touch controller reset (TP_RST) is connected to 3V3 (always high)
- Touch interrupt (TP_INT) is not used in current implementation
- Display resolution: 466x466 pixels (circular)
- Color format: RGB565 (16-bit)
