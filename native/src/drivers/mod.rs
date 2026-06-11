pub mod dry_run;
pub mod mcp2221;

use crate::config::RuntimeConfig;
use crate::error::Result;
use crate::model::{ChannelState, Frame};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChannelLevels {
    pub green: f32,
    pub yellow: f32,
    pub red: f32,
}

impl ChannelLevels {
    pub const fn off() -> Self {
        Self {
            green: 0.0,
            yellow: 0.0,
            red: 0.0,
        }
    }

    pub fn from_frame(frame: &Frame) -> Self {
        let brightness = frame.brightness_hint.unwrap_or(1.0).clamp(0.0, 1.0);
        Self {
            green: if frame.green_on { brightness } else { 0.0 },
            yellow: if frame.yellow_on { brightness } else { 0.0 },
            red: if frame.red_on { brightness } else { 0.0 },
        }
    }

    pub fn from_state(state: ChannelState) -> Self {
        Self {
            green: if state.green_on { 1.0 } else { 0.0 },
            yellow: if state.yellow_on { 1.0 } else { 0.0 },
            red: if state.red_on { 1.0 } else { 0.0 },
        }
    }
}

pub trait LightDriver {
    fn write_levels(&mut self, levels: ChannelLevels) -> Result<()>;

    fn off(&mut self) -> Result<()> {
        self.write_levels(ChannelLevels::off())
    }

    fn supports_brightness(&self) -> bool {
        false
    }
}

pub fn create_driver(config: &RuntimeConfig, dry_run: bool) -> Result<Box<dyn LightDriver>> {
    if dry_run {
        return Ok(Box::new(dry_run::DryRunDriver::stdout()));
    }
    Ok(Box::new(mcp2221::Mcp2221Driver::open(
        config.hardware.clone(),
    )?))
}
