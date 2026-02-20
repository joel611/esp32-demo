// src/ft3168.rs
use esp_idf_svc::hal::i2c::I2cDriver;

const ADDR: u8 = 0x38;

pub struct Ft3168<'d> {
    i2c: I2cDriver<'d>,
}

impl<'d> Ft3168<'d> {
    pub fn new(i2c: I2cDriver<'d>) -> Self {
        Self { i2c }
    }

    /// Switch FT3168 to normal mode. Call once after power-on.
    /// The 200 ms delay lets the controller stabilise.
    pub fn init(&mut self) -> Result<(), esp_idf_svc::sys::EspError> {
        std::thread::sleep(std::time::Duration::from_millis(200));
        self.i2c.write(ADDR, &[0x00, 0x00], 1000)?;
        Ok(())
    }

    /// Returns `Some((x, y))` if a finger is currently touching the screen,
    /// `None` if no touch is active.
    pub fn read_touch(&mut self) -> Result<Option<(u16, u16)>, esp_idf_svc::sys::EspError> {
        let mut count = [0u8; 1];
        self.i2c.write_read(ADDR, &[0x02], &mut count, 1000)?;
        if count[0] == 0 {
            return Ok(None);
        }

        let mut buf = [0u8; 4];
        self.i2c.write_read(ADDR, &[0x03], &mut buf, 1000)?;

        // Register layout: buf[0] bits[3:0] = X[11:8], buf[1] = X[7:0]
        //                  buf[2] bits[3:0] = Y[11:8], buf[3] = Y[7:0]
        let x = (((buf[0] & 0x0F) as u16) << 8) | buf[1] as u16;
        let y = (((buf[2] & 0x0F) as u16) << 8) | buf[3] as u16;

        Ok(Some((x.min(465), y.min(465))))
    }
}
