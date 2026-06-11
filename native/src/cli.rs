use std::collections::BTreeMap;
use std::io::{self, Read};
use std::thread;
use std::time::Duration;

use clap::{Parser, Subcommand};

use crate::config::RuntimeConfig;
use crate::drivers::{create_driver, ChannelLevels};
use crate::error::{Result, SignalLightError};
use crate::hooks::{claude_code, codex, owner_pid_from_payload_or_env};
use crate::install_hooks;
use crate::model::{Frame, RuntimeCommand, SignalDefinition};
use crate::runtime::{ipc, server};
use crate::signals;

#[derive(Debug, Parser)]
#[command(name = "signal-light-native")]
#[command(about = "Native runtime for AI agent signal lights.")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    List,
    Status,
    Play {
        signal: String,
        #[arg(long)]
        dry_run: bool,
        #[arg(long, default_value_t = 1.0)]
        speed: f32,
        #[arg(long)]
        quiet: bool,
    },
    CodexHook {
        event: Option<String>,
        #[arg(long = "event")]
        event_option: Option<String>,
        #[arg(long)]
        dry_run: bool,
    },
    ClaudeCodeHook {
        event: Option<String>,
        #[arg(long = "event")]
        event_option: Option<String>,
        #[arg(long)]
        dry_run: bool,
    },
    InstallHooks {
        #[arg(long = "agent")]
        agents: Vec<String>,
        #[arg(long)]
        all: bool,
        #[arg(short = 'y', long)]
        yes: bool,
        #[arg(long)]
        dry_run: bool,
    },
    Server {
        #[arg(long)]
        dry_run: bool,
    },
    Test {
        #[arg(long)]
        dry_run: bool,
    },
}

pub fn run_from_env() -> i32 {
    match run() {
        Ok(code) => code,
        Err(error) => {
            eprintln!("{error}");
            error.exit_code()
        }
    }
}

fn run() -> Result<i32> {
    let cli = Cli::parse();
    let config = RuntimeConfig::from_env()?;
    match cli.command {
        Commands::List => list_signals(),
        Commands::Status => show_status(&config),
        Commands::Play {
            signal,
            dry_run,
            speed,
            quiet,
        } => play_signal(&config, &signal, dry_run, speed, quiet),
        Commands::CodexHook {
            event,
            event_option,
            dry_run,
        } => run_codex_hook(&config, event_option.or(event), dry_run),
        Commands::ClaudeCodeHook {
            event,
            event_option,
            dry_run,
        } => run_claude_code_hook(&config, event_option.or(event), dry_run),
        Commands::InstallHooks {
            agents,
            all,
            yes,
            dry_run,
        } => install_hooks::run_cli(agents, all, yes, dry_run),
        Commands::Server { dry_run } => {
            server::run(config, dry_run)?;
            Ok(0)
        }
        Commands::Test { dry_run } => run_test(&config, dry_run),
    }
}

fn list_signals() -> Result<i32> {
    println!("Signal language:");
    for signal in signals::definitions().values() {
        println!("- {}: {} {}", signal.name, signal.summary, signal.attention);
    }
    Ok(0)
}

fn show_status(config: &RuntimeConfig) -> Result<i32> {
    let snapshot = ipc::status(config)?;
    println!("{}", serde_json::to_string_pretty(&snapshot)?);
    Ok(0)
}

fn play_signal(
    config: &RuntimeConfig,
    signal_name: &str,
    dry_run: bool,
    speed: f32,
    quiet: bool,
) -> Result<i32> {
    let signal = signals::signal(signal_name)
        .ok_or_else(|| SignalLightError::InvalidSignal(format!("Unknown signal: {signal_name}")))?;

    if !quiet {
        println!("Playing {}: {}", signal.name, signal.summary);
    }

    if dry_run {
        let mut driver = create_driver(config, true)?;
        preview_signal(
            driver.as_mut(),
            signal,
            speed,
            if signal.repeat { Some(2) } else { None },
        )?;
        return Ok(0);
    }

    let response = ipc::request(
        config,
        &RuntimeCommand {
            action: "direct_signal".to_string(),
            session_key: None,
            signal_name: Some(signal_name.to_string()),
            owner_pid: None,
            speed: Some(speed),
            reply_to: None,
        },
        true,
    )?;
    if !quiet {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "aggregate": response.aggregate,
                "display_signal": response.display_signal
            }))?
        );
    }
    Ok(0)
}

fn run_codex_hook(
    config: &RuntimeConfig,
    event_override: Option<String>,
    dry_run: bool,
) -> Result<i32> {
    let stdin_text = read_stdin()?;
    let env = env_map();
    let dry_run = dry_run || env_flag(&env, "SIGNAL_LIGHT_DRY_RUN");
    let argv = hook_argv("signal-light-native", event_override);
    let input = codex::read_input(&argv, &stdin_text, &env);
    let signal_name = codex::choose_signal(&input);
    let session_key = codex::session_key(&input, &env);
    let owner_pid = owner_pid_from_payload_or_env(&input.payload, &env);
    play_hook_signal(config, &signal_name, &session_key, owner_pid, dry_run)
}

fn run_claude_code_hook(
    config: &RuntimeConfig,
    event_override: Option<String>,
    dry_run: bool,
) -> Result<i32> {
    let stdin_text = read_stdin()?;
    let env = env_map();
    let dry_run = dry_run || env_flag(&env, "SIGNAL_LIGHT_DRY_RUN");
    let argv = hook_argv("signal-light-native", event_override);
    let input = claude_code::read_input(&argv, &stdin_text);
    let signal_name = claude_code::choose_signal(&input);
    let session_key = claude_code::session_key(&input, &env);
    let owner_pid = owner_pid_from_payload_or_env(&input.payload, &env);
    play_hook_signal(config, &signal_name, &session_key, owner_pid, dry_run)
}

fn play_hook_signal(
    config: &RuntimeConfig,
    signal_name: &str,
    session_key: &str,
    owner_pid: Option<u32>,
    dry_run: bool,
) -> Result<i32> {
    if !(signals::is_public_signal(signal_name) || signals::is_hook_control_signal(signal_name)) {
        return Err(SignalLightError::InvalidSignal(format!(
            "Unknown signal: {signal_name}"
        )));
    }

    if dry_run {
        if let Some(signal) = signals::signal(signal_name) {
            let mut driver = create_driver(config, true)?;
            preview_signal(
                driver.as_mut(),
                signal,
                1.0,
                if signal.repeat { Some(2) } else { None },
            )?;
        }
        return Ok(0);
    }

    ipc::request(
        config,
        &RuntimeCommand {
            action: "session_signal".to_string(),
            session_key: Some(session_key.to_string()),
            signal_name: Some(signal_name.to_string()),
            owner_pid,
            speed: Some(1.0),
            reply_to: None,
        },
        true,
    )?;
    Ok(0)
}

fn run_test(config: &RuntimeConfig, dry_run: bool) -> Result<i32> {
    let test_signal = SignalDefinition {
        name: "test",
        summary: "red/yellow/green wiring test",
        attention: "",
        attention_level: crate::model::AttentionLevel::Working,
        frames: vec![
            Frame::solid(false, false, true, 350),
            Frame::solid(false, true, false, 350),
            Frame::solid(true, false, false, 350),
            Frame::solid(true, true, true, 350),
        ],
        loops: 2,
        steady_state: None,
        repeat: false,
    };
    let mut driver = create_driver(config, dry_run)?;
    preview_signal(driver.as_mut(), &test_signal, 1.0, Some(test_signal.loops))?;
    Ok(0)
}

fn preview_signal(
    driver: &mut dyn crate::drivers::LightDriver,
    signal: &SignalDefinition,
    speed: f32,
    cycles: Option<u32>,
) -> Result<()> {
    let loops = cycles.unwrap_or(signal.loops).max(1);
    for _ in 0..loops {
        for frame in &signal.frames {
            render_preview_frame(driver, frame)?;
            thread::sleep(Duration::from_millis(
                ((frame.duration_ms as f32) * speed.max(0.05)) as u64,
            ));
        }
    }
    if let Some(steady_state) = signal.steady_state {
        driver.write_levels(ChannelLevels::from_state(steady_state))?;
    } else {
        driver.off()?;
    }
    Ok(())
}

fn render_preview_frame(driver: &mut dyn crate::drivers::LightDriver, frame: &Frame) -> Result<()> {
    let mut levels = ChannelLevels::from_frame(frame);
    if !driver.supports_brightness() {
        levels = ChannelLevels {
            green: if levels.green > 0.0 { 1.0 } else { 0.0 },
            yellow: if levels.yellow > 0.0 { 1.0 } else { 0.0 },
            red: if levels.red > 0.0 { 1.0 } else { 0.0 },
        };
    }
    if levels.green == 0.0 && levels.yellow == 0.0 && levels.red == 0.0 {
        driver.off()
    } else {
        driver.write_levels(levels)
    }
}

fn read_stdin() -> Result<String> {
    let mut buffer = String::new();
    io::stdin().read_to_string(&mut buffer)?;
    Ok(buffer)
}

fn env_map() -> BTreeMap<String, String> {
    std::env::vars().collect()
}

fn hook_argv(binary_name: &str, event_override: Option<String>) -> Vec<String> {
    match event_override {
        Some(event_name) => vec![binary_name.to_string(), "--event".to_string(), event_name],
        None => vec![binary_name.to_string()],
    }
}

fn env_flag(env: &BTreeMap<String, String>, key: &str) -> bool {
    env.get(key).is_some_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}
