use std::io::{self, Write};

use crate::drivers::{ChannelLevels, LightDriver};
use crate::error::Result;

pub struct DryRunDriver {
    writer: Box<dyn Write + Send>,
}

impl DryRunDriver {
    pub fn stdout() -> Self {
        Self {
            writer: Box::new(io::stdout()),
        }
    }

    pub fn line_for_levels(levels: ChannelLevels) -> String {
        let whole_number = [levels.green, levels.yellow, levels.red]
            .into_iter()
            .all(|value| matches!(value, 0.0 | 1.0));
        if whole_number {
            return format!(
                "green={} yellow={} red={}",
                levels.green as u8, levels.yellow as u8, levels.red as u8
            );
        }
        format!(
            "green={:.2} yellow={:.2} red={:.2}",
            levels.green, levels.yellow, levels.red
        )
    }
}

impl LightDriver for DryRunDriver {
    fn write_levels(&mut self, levels: ChannelLevels) -> Result<()> {
        writeln!(self.writer, "{}", Self::line_for_levels(levels))?;
        self.writer.flush()?;
        Ok(())
    }

    fn supports_brightness(&self) -> bool {
        true
    }
}
