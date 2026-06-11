use crate::drivers::{ChannelLevels, LightDriver};
use crate::error::{Result, SignalLightError};
use crate::model::HardwareMapping;

pub struct Mcp2221Driver {
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    device: hidapi::HidDevice,
    mapping: HardwareMapping,
}

impl Mcp2221Driver {
    pub fn open(mapping: HardwareMapping) -> Result<Self> {
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        {
            let api = hidapi::HidApi::new().map_err(|error| {
                SignalLightError::Hardware(format!("Failed to initialize HID API: {error}"))
            })?;
            let device = api.open(0x04D8, 0x00DD).map_err(|error| {
                SignalLightError::Hardware(format!("Failed to open MCP2221A device: {error}"))
            })?;
            let mut driver = Self { device, mapping };
            driver.configure_gpio_outputs()?;
            driver.off()?;
            Ok(driver)
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            let _ = mapping;
            Err(SignalLightError::Hardware(
                "MCP2221A is only supported on macOS and Linux.".to_string(),
            ))
        }
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fn configure_gpio_outputs(&mut self) -> Result<()> {
        const CMD_GET_SRAM_SETTINGS: u8 = 0x61;
        const CMD_SET_SRAM_SETTINGS: u8 = 0x60;
        const ALTER_GPIO_CONF: u8 = 1 << 7;
        const GPIO_OUT_VAL_1: u8 = 1 << 4;
        const GPIO_DIR_OUT: u8 = 0;
        const GPIO_FUNC_GPIO: u8 = 0;
        const PRESERVE_CLK_OUTPUT: u8 = 0;
        const PRESERVE_DAC_VALUE: u8 = 0;
        const PRESERVE_INT_CONF: u8 = 0;
        const ALTER_DAC_REF: u8 = 1 << 7;
        const ALTER_ADC_REF: u8 = 1 << 7;

        let response = self.send_command(&[CMD_GET_SRAM_SETTINGS])?;
        let mut gp = [response[18], response[19], response[20], response[21]];
        for pin_name in [
            self.mapping.green_pin.as_str(),
            self.mapping.yellow_pin.as_str(),
            self.mapping.red_pin.as_str(),
        ] {
            let pin_index = parse_pin(pin_name)?;
            gp[pin_index] = GPIO_FUNC_GPIO
                | GPIO_DIR_OUT
                | if self.physical_value(false) {
                    GPIO_OUT_VAL_1
                } else {
                    0
                };
        }
        let command = [
            CMD_SET_SRAM_SETTINGS,
            0,
            PRESERVE_CLK_OUTPUT,
            ALTER_DAC_REF,
            PRESERVE_DAC_VALUE,
            ALTER_ADC_REF,
            PRESERVE_INT_CONF,
            ALTER_GPIO_CONF,
            gp[0],
            gp[1],
            gp[2],
            gp[3],
        ];
        self.send_command(&command)?;
        Ok(())
    }

    #[cfg(any(target_os = "macos", target_os = "linux"))]
    fn send_command(&self, command: &[u8]) -> Result<[u8; 64]> {
        const PACKET_SIZE: usize = 64;
        let mut packet = [0_u8; PACKET_SIZE + 1];
        packet[0] = 0;
        packet[1..1 + command.len()].copy_from_slice(command);
        self.device.write(&packet).map_err(|error| {
            SignalLightError::Hardware(format!("Failed to write MCP2221A command: {error}"))
        })?;
        let mut response = [0_u8; PACKET_SIZE];
        let read = self
            .device
            .read_timeout(&mut response, 1_000)
            .map_err(|error| {
                SignalLightError::Hardware(format!("Failed to read MCP2221A response: {error}"))
            })?;
        if read == 0 {
            return Err(SignalLightError::Timeout(
                "Timed out waiting for MCP2221A response.".to_string(),
            ));
        }
        if response[1] != 0 {
            return Err(SignalLightError::Hardware(format!(
                "MCP2221A returned error status 0x{:02x}.",
                response[1]
            )));
        }
        Ok(response)
    }

    fn physical_value(&self, logical_on: bool) -> bool {
        logical_to_physical(self.mapping.active_low, logical_on)
    }
}

impl LightDriver for Mcp2221Driver {
    fn write_levels(&mut self, levels: ChannelLevels) -> Result<()> {
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        {
            const CMD_SET_GPIO_OUTPUT_VALUES: u8 = 0x50;
            let mut command = [0_u8; 16];
            command[0] = CMD_SET_GPIO_OUTPUT_VALUES;
            for (pin_name, logical_on) in [
                (&self.mapping.green_pin, levels.green > 0.0),
                (&self.mapping.yellow_pin, levels.yellow > 0.0),
                (&self.mapping.red_pin, levels.red > 0.0),
            ] {
                let pin_index = parse_pin(pin_name)?;
                let value_index = 2 + (pin_index * 4);
                command[value_index] = 1;
                command[value_index + 1] = u8::from(self.physical_value(logical_on));
            }
            let response = self.send_command(&command)?;
            for pin_name in [
                self.mapping.green_pin.as_str(),
                self.mapping.yellow_pin.as_str(),
                self.mapping.red_pin.as_str(),
            ] {
                let pin_index = parse_pin(pin_name)?;
                let status_index = 3 + (pin_index * 4);
                if response[status_index] == 0xEE {
                    return Err(SignalLightError::Hardware(format!(
                        "Pin {} is not assigned to GPIO output.",
                        pin_name.to_ascii_uppercase()
                    )));
                }
            }
            Ok(())
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            let _ = levels;
            Err(SignalLightError::Hardware(
                "MCP2221A is only supported on macOS and Linux.".to_string(),
            ))
        }
    }
}

fn parse_pin(pin_name: &str) -> Result<usize> {
    match pin_name.trim().to_ascii_lowercase().as_str() {
        "gp0" => Ok(0),
        "gp1" => Ok(1),
        "gp2" => Ok(2),
        "gp3" => Ok(3),
        _ => Err(SignalLightError::Configuration(format!(
            "Unsupported MCP2221A pin mapping: {pin_name}"
        ))),
    }
}

pub fn logical_to_physical(active_low: bool, logical_on: bool) -> bool {
    if active_low {
        !logical_on
    } else {
        logical_on
    }
}
