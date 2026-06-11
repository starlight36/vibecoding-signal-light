use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ChannelState {
    pub green_on: bool,
    pub yellow_on: bool,
    pub red_on: bool,
}

impl ChannelState {
    pub const fn new(green_on: bool, yellow_on: bool, red_on: bool) -> Self {
        Self {
            green_on,
            yellow_on,
            red_on,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    pub green_on: bool,
    pub yellow_on: bool,
    pub red_on: bool,
    pub duration_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brightness_hint: Option<f32>,
}

impl Frame {
    pub const fn solid(green_on: bool, yellow_on: bool, red_on: bool, duration_ms: u64) -> Self {
        Self {
            green_on,
            yellow_on,
            red_on,
            duration_ms,
            brightness_hint: None,
        }
    }

    pub const fn with_brightness(
        green_on: bool,
        yellow_on: bool,
        red_on: bool,
        duration_ms: u64,
        brightness_hint: f32,
    ) -> Self {
        Self {
            green_on,
            yellow_on,
            red_on,
            duration_ms,
            brightness_hint: Some(brightness_hint),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttentionLevel {
    Idle,
    Working,
    Attention,
    Permission,
    Blocked,
    Completion,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SignalDefinition {
    pub name: &'static str,
    pub summary: &'static str,
    pub attention: &'static str,
    pub attention_level: AttentionLevel,
    pub frames: Vec<Frame>,
    pub loops: u32,
    pub steady_state: Option<ChannelState>,
    pub repeat: bool,
}

impl SignalDefinition {
    pub fn duration_ms(&self, speed_factor: f32) -> u64 {
        let scale = speed_factor.max(0.05);
        let per_loop: u64 = self
            .frames
            .iter()
            .map(|frame| ((frame.duration_ms as f32) * scale).max(0.0) as u64)
            .sum();
        per_loop.saturating_mul(self.loops as u64)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionRecord {
    pub signal: String,
    pub updated_at: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner_pid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner_pid_source: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct StateDocument {
    #[serde(default)]
    pub sessions: BTreeMap<String, SessionRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direct_signal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_signal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_pid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeSnapshot {
    pub aggregate: String,
    pub display_signal: String,
    pub sessions: BTreeMap<String, SessionRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_pid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeCommand {
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner_pid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuntimeResponse {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signal: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aggregate: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_signal: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub sessions: BTreeMap<String, SessionRecord>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_pid: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HardwareMapping {
    pub green_pin: String,
    pub yellow_pin: String,
    pub red_pin: String,
    pub active_low: bool,
}

impl Default for HardwareMapping {
    fn default() -> Self {
        Self {
            green_pin: "gp0".to_string(),
            yellow_pin: "gp1".to_string(),
            red_pin: "gp2".to_string(),
            active_low: true,
        }
    }
}
