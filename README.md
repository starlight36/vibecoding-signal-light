# Vibecoding Signal Light

> A real traffic light for AI agents.  
> 给 AI Agent 一个看得见的状态灯。

Vibecoding Signal Light turns a small red/yellow/green traffic signal model into an ambient status display for Codex, Claude Code, and other local AI coding agents. When the agent is working, waiting, blocked, or asking for permission, the light on your desk changes with it.

It is deliberately simple: glance at the lamp, know whether you should keep flowing or look back at your agent.

Vibecoding Signal Light 把一个红、黄、绿三色交通信号灯模型变成 AI 编程助手的实体状态面板。Codex、Claude Code 或其他本地 Agent 开始工作、请求权限、遇到阻塞时，桌上的信号灯会同步变化。

它的目标不是炫技，而是让 AI Agent 从屏幕里的文字流，变成房间里能被一眼感知的工作伙伴。

## Demo / 示例

![Vibecoding Signal Light demo: green idle state mounted beside a laptop](docs/images/demo.jpg)

The reference build mounted beside a laptop, showing the steady green idle state.

参考实物安装在笔记本旁边，图中是绿灯常亮的空闲状态。

## Why This Exists / 为什么做这个

AI coding agents are getting more autonomous, but their state is still trapped inside a terminal or chat window. That creates two awkward modes:

- You keep checking the agent too often and break your own focus.
- You forget it is waiting for permission, a result review, or a failure recovery.

This project gives the agent a physical presence:

- Green means you can relax.
- A slow three-color cycle means the agent is busy.
- Yellow means the agent explicitly needs a look.
- Red means stop what you are doing and unblock it.

AI 编程助手越来越能自己跑命令、改文件、开子任务，但它的状态通常还困在终端或聊天窗口里。于是你要么反复切回去看，打断自己的注意力；要么忘了它正在等权限、等你读结果、或者已经失败。

这个项目给 Agent 一个真实存在的环境信号：

- 绿灯：没事，继续你的事。
- 绿黄红慢速循环：Agent 正在工作。
- 黄闪：Agent 明确需要你看一眼或继续。
- 红闪：需要马上处理，通常是权限、阻塞或失败。

## Hardware / 硬件

The current reference build uses:

| Part | Description |
| --- | --- |
| MCP2221A USB GPIO adapter | Drives the traffic light from a Mac/Linux machine over USB |
| 3-light traffic signal model | Red, yellow, and green LEDs or lamp modules |
| Rust native runtime | Local GPIO control, no Python environment or network service required |

当前参考硬件：

| 硬件 | 说明 |
| --- | --- |
| MCP2221A USB GPIO 转接板 | 通过 USB 从电脑控制 GPIO |
| 三色交通信号灯模型 | 红、黄、绿三路 LED 或灯模块 |
| Rust 原生运行时 | 本地控制 GPIO，不需要 Python 环境或额外云服务 |

Default wiring is active-low:

| Signal | MCP2221A pin | Meaning |
| --- | --- | --- |
| Green | `gp0` | Idle |
| Yellow | `gp1` | Attention |
| Red | `gp2` | Permission, blocked, or failed |
| Active level | GPIO `LOW` | Light on |

默认接线是低电平点亮：

| 灯 | MCP2221A 引脚 | 含义 |
| --- | --- | --- |
| 绿灯 | `gp0` | 空闲 |
| 黄灯 | `gp1` | 需要关注 |
| 红灯 | `gp2` | 权限、阻塞或失败 |
| 有效电平 | GPIO `LOW` | 灯亮 |

### Wiring / 接线

The reference build uses a common-anode, active-low LED-style wiring. Each light has its own current-limiting resistor unless your traffic light module already includes one.

参考实物使用公共正极、低电平点亮的 LED 接法。每一路灯都应该串联独立限流电阻，除非你的交通灯模块已经内置电阻。

```text
MCP2221A 3.3V  ────────────────┬── Green LED anode / 绿灯正极
                               ├── Yellow LED anode / 黄灯正极
                               └── Red LED anode / 红灯正极

Green LED cathode / 绿灯负极   ── 220Ω-1kΩ ── GP0
Yellow LED cathode / 黄灯负极  ── 220Ω-1kΩ ── GP1
Red LED cathode / 红灯负极     ── 220Ω-1kΩ ── GP2
```

```mermaid
flowchart LR
    V33["MCP2221A 3.3V"] --> COMMON["Common anode / 公共正极"]
    COMMON --> GLED["Green LED / 绿灯"]
    COMMON --> YLED["Yellow LED / 黄灯"]
    COMMON --> RLED["Red LED / 红灯"]
    GLED --> GR["220Ω-1kΩ"] --> GP0["GP0"]
    YLED --> YR["220Ω-1kΩ"] --> GP1["GP1"]
    RLED --> RR["220Ω-1kΩ"] --> GP2["GP2"]
```

In this mode, the MCP2221A GPIO pin sinks current:

- GPIO `HIGH`: light off
- GPIO `LOW`: light on

这种模式下 MCP2221A GPIO 负责下拉电流：

- GPIO `HIGH`：灯灭
- GPIO `LOW`：灯亮

If your signal model is common-cathode or active-high, wire each GPIO through a resistor to the LED anode, connect the cathodes to `GND`, and set:

```bash
export SIGNAL_LIGHT_ACTIVE_LOW=0
```

如果你的灯是公共负极或高电平点亮，则应让每个 GPIO 通过限流电阻接到对应 LED 正极，LED 负极接 `GND`，并设置：

```bash
export SIGNAL_LIGHT_ACTIVE_LOW=0
```

Important: MCP2221A GPIO pins are for small LED loads only. If your traffic light uses 5V/12V lamps, LED strips, relays, or anything above the GPIO current limit, use a transistor, MOSFET, relay module, or dedicated LED driver between the MCP2221A and the light.

注意：MCP2221A GPIO 只适合直接驱动小电流 LED。若你的信号灯是 5V/12V 灯组、灯带、继电器，或电流超过 GPIO 能力，请在 MCP2221A 和灯之间增加三极管、MOSFET、继电器模块或专用 LED 驱动。

You can override the wiring:

```bash
export SIGNAL_LIGHT_GREEN_PIN=gp0
export SIGNAL_LIGHT_YELLOW_PIN=gp1
export SIGNAL_LIGHT_RED_PIN=gp2
export SIGNAL_LIGHT_ACTIVE_LOW=1
```

Set `SIGNAL_LIGHT_ACTIVE_LOW=0` if your traffic light is wired active-high.

## Lamp Language / 灯语

The language is intentionally small and persistent. The current light should always describe the current state.

灯语刻意保持简单，并且状态会持续显示。你不需要记复杂动画，只要看当前灯效。

| Light / 灯效 | Agent state / Agent 状态 | Human action / 你该做什么 |
| --- | --- | --- |
| Steady green / 绿灯常亮 | Idle / 空闲 | Nothing / 不用管 |
| Slow green-yellow-red cycle / 绿黄红慢速循环 | Working / 正在思考、跑工具、改文件或测试 | Wait / 等它跑 |
| Flashing yellow / 黄灯闪烁 | Explicit attention / 明确需要你读结果或继续 | Look when convenient / 有空看一眼 |
| Flashing red / 红灯闪烁 | Permission, blocked, or failed / 需要权限、阻塞或失败 | Look now / 马上处理 |
| Off / 全灭 | Manual clear / 手动清除 | Nothing / 不用管 |

The work cycle avoids software PWM on plain GPIO hardware, because USB GPIO timing can create visible flicker. On the MCP2221A reference build, the work state is a calm, slow three-color cycle. If you later add a driver that supports real brightness control, the same pattern can be rendered as a soft pulse.

当前 MCP2221A GPIO 参考实现不会用软件 PWM 模拟呼吸灯，因为 USB GPIO 的时序抖动会造成肉眼可见的频闪。工作态默认是安静的三色慢速循环。如果未来换成真正支持亮度控制的驱动，同一套模式可以渲染成柔和脉冲。

## Features / 功能亮点

- Physical ambient status for AI agents.
- Codex hook adapter.
- Claude Code hook adapter.
- Session-aware aggregation for multiple concurrent agent sessions.
- Red and yellow alerts are never hidden by another session starting work.
- Local display server keeps animations persistent while hooks return quickly.
- Dry-run mode for testing without hardware.
- Environment-based GPIO mapping for custom builds.

- 给 AI Agent 一个实体环境状态灯。
- 支持 Codex hook。
- 支持 Claude Code hook。
- 支持多个 Agent 会话并发时的状态聚合。
- 红灯/黄灯告警不会被另一个会话的工作态覆盖。
- 本地显示 server 保持灯效持续运行，hook 本身快速返回。
- 支持无硬件 dry-run 预览。
- 支持通过环境变量调整 GPIO 接线。

## Migrating From the Earlier Python Version / 从早期 Python 版本迁移到最新版 Rust 运行时

Signal Light has moved from the earlier Python/uv script runtime to the latest Rust-native runtime. The migration was made because this project now behaves more like a small native hardware utility than an application script: hooks need to return quickly, one local server needs to own the persistent lamp animation, and users should not need to debug Python/uv environments just to drive a desk light. See [PR #8](https://github.com/starlight36/vibecoding-signal-light/pull/8) for the full migration context and implementation details.

Signal Light 已经从早期的 Python/uv 脚本运行时迁移到最新版 Rust 原生运行时。迁移的原因是：这个项目现在更像一个小型原生硬件工具，而不是普通应用脚本；hook 需要快速返回，本地 server 需要持续持有灯的动画和状态，用户也不应该为了驱动桌面信号灯去排查 Python/uv 环境问题。完整迁移背景和实现细节见 [PR #8](https://github.com/starlight36/vibecoding-signal-light/pull/8)。

The current wrapper scripts no longer run `python -m signal_light`, do not require `uv sync`, and do not use a repository `.venv`. Build the Rust binary or download a release archive before running the commands below.

当前 wrapper 不再执行 `python -m signal_light`，不需要 `uv sync`，也不依赖仓库里的 `.venv`。运行下面的命令前，请先构建 Rust 二进制，或下载对应平台的 release 包。

If you still need the old Python implementation, use the [`legacy-python-main`](https://github.com/starlight36/vibecoding-signal-light/tree/legacy-python-main) branch.

如果你仍然需要旧的 Python 实现，可以查看 [`legacy-python-main`](https://github.com/starlight36/vibecoding-signal-light/tree/legacy-python-main) 分支。

If you are upgrading from the Python version:

1. Build or install the native binary:

   ```bash
   cargo build --manifest-path native/Cargo.toml --release
   ```

2. Reinstall or repair hooks so Codex and Claude Code configs point at the native wrappers:

   ```bash
   ./scripts/install-hooks --all -y
   ```

3. Smoke-test the new runtime without hardware:

   ```bash
   ./scripts/signal-light play working --dry-run
   ./scripts/signal-light status
   ```

4. The old Python project files (`pyproject.toml`, `uv.lock`, `.python-version`, and `signal_light/`) have been removed. Local `.venv` or `.pytest_cache` directories can be deleted if you only used them for Signal Light.

如果你正在从 Python 版本升级：

1. 构建或安装原生二进制：

   ```bash
   cargo build --manifest-path native/Cargo.toml --release
   ```

2. 重新安装或修复 hooks，让 Codex 和 Claude Code 配置指向新的原生 wrapper：

   ```bash
   ./scripts/install-hooks --all -y
   ```

3. 先用 dry-run 做一次无硬件烟测：

   ```bash
   ./scripts/signal-light play working --dry-run
   ./scripts/signal-light status
   ```

4. 旧的 Python 项目文件（`pyproject.toml`、`uv.lock`、`.python-version`、`signal_light/`）已经移除。如果本地 `.venv` 或 `.pytest_cache` 只是给 Signal Light 用的，也可以删掉。

## Quick Start / 快速开始

Build the native runtime:

```bash
cargo build --manifest-path native/Cargo.toml --release
```

Or install the packaged release with Homebrew:

```bash
brew install starlight36/tap/signal-light
```

Or download a prebuilt release archive for your platform:

- `signal-light-<version>-macos-aarch64.tar.gz`
- `signal-light-<version>-macos-x86_64.tar.gz`
- `signal-light-<version>-linux-amd64.tar.gz`
- `signal-light-<version>-linux-arm64.tar.gz`

Each archive contains `bin/signal-light-native` plus the `scripts/` wrappers, so the wrapper commands below work from the unpacked directory without extra setup.

List the signal language:

```bash
./scripts/signal-light list
```

Preview without hardware:

```bash
./scripts/signal-light play working --dry-run
./scripts/signal-light play attention --dry-run
./scripts/signal-light play permission --dry-run
```

Run a wiring test on the real MCP2221A setup:

```bash
./scripts/signal-light test
```

Expected hardware-test outcome on the reference build:

- default active-low wiring: red -> yellow -> green -> all three on -> command exits
- active-high wiring: export `SIGNAL_LIGHT_ACTIVE_LOW=0` first, then expect the same logical order
- missing or busy hardware: the command exits non-zero with a concise MCP2221A diagnostic instead of hanging

Play real signals:

```bash
./scripts/signal-light play working
./scripts/signal-light play permission
./scripts/signal-light play idle
```

The wrapper scripts require a built native binary. They look first at `SIGNAL_LIGHT_NATIVE_BIN`, then `bin/signal-light-native`, then `native/target/release/signal-light-native`, then `native/target/debug/signal-light-native`:

```bash
export SIGNAL_LIGHT_NATIVE_BIN=/absolute/path/to/signal-light-native
```

If no native binary is available, the wrappers exit with a concise build instruction instead of falling back to Python.

Runtime state is stored in a per-user directory by default: `$XDG_STATE_HOME/signal-light` when set, `~/Library/Application Support/signal-light` on macOS, or `~/.local/state/signal-light` on Linux. Override it with `SIGNAL_LIGHT_STATE_DIR` if you need a custom location.

## Codex Integration / Codex 集成

The easiest way to install or repair local hooks is the built-in wizard:

```bash
./scripts/install-hooks
./scripts/install-hooks --all -y
./scripts/install-hooks --agent codex --agent claude-code --agent opencode -y
```

The wizard detects supported local agents, validates the current hook files, creates timestamped backups, and installs only the Signal Light hook entries while keeping other hooks on the same events.

The hook installer is implemented in the native binary as `signal-light-native install-hooks`; the `./scripts/install-hooks` wrapper uses that command directly.

The first hook or `play` command auto-starts a local Signal Light server process. That server owns the shared display state, the per-session state, and the animation loop, keeping the single physical lamp in sync for all local agent clients. `status` reports both the session aggregate and the actual `display_signal` currently owned by the server.

安装或修复本地 hook 最简单的方式是内置向导：

```bash
./scripts/install-hooks
./scripts/install-hooks --all -y
./scripts/install-hooks --agent codex --agent claude-code --agent opencode -y
```

向导会识别已支持的本地 Agent，检查当前 hook 文件，写入前创建带时间戳的备份，并且只安装 Signal Light 自己的 hook 条目，保留同一事件下已有的其它 hook。

Codex hooks can call the wrapper with the event name:

```bash
./scripts/codex-signal-hook UserPromptSubmit
./scripts/codex-signal-hook PreToolUse
./scripts/codex-signal-hook PermissionRequest
./scripts/codex-signal-hook Stop
```

Recommended hook mapping:

| Codex event | Signal behavior |
| --- | --- |
| `SessionStart` | Green idle |
| `UserPromptSubmit` | Working cycle |
| `PreToolUse` | Working cycle |
| `PostToolUse` | Working cycle |
| `PermissionRequest` | Red flashing |
| `Stop` | Green completion cue, then aggregate state without that session's normal work |
| `SessionEnd` | Session cleanup; if still tracked, brief green completion blink, then current aggregate state |

See [docs/LAMP_LANGUAGE.md](docs/LAMP_LANGUAGE.md) for a complete `~/.codex/hooks.json` example.

Codex hook 可以直接把事件名传给 wrapper：

```bash
./scripts/codex-signal-hook UserPromptSubmit
./scripts/codex-signal-hook PreToolUse
./scripts/codex-signal-hook PermissionRequest
./scripts/codex-signal-hook Stop
```

推荐映射：

| Codex 事件 | 灯效行为 |
| --- | --- |
| `SessionStart` | 绿灯空闲 |
| `UserPromptSubmit` | 工作循环 |
| `PreToolUse` | 工作循环 |
| `PostToolUse` | 工作循环 |
| `PermissionRequest` | 红灯闪烁 |
| `Stop` | 绿灯提示完成，然后恢复去掉该会话普通工作态后的聚合状态 |
| `SessionEnd` | 会话清理；如果该会话仍被跟踪，则绿灯短闪提示完成，然后恢复当前聚合状态 |

完整 `~/.codex/hooks.json` 示例见 [docs/LAMP_LANGUAGE.md](docs/LAMP_LANGUAGE.md)。

## Claude Code Integration / Claude Code 集成

Claude Code sends hook data as JSON on stdin, so the wrapper usually needs no event argument:

```bash
echo '{"event":"PreToolUse","session_id":"demo"}' | ./scripts/claude-code-signal-hook
echo '{"event":"PermissionRequest","session_id":"demo"}' | ./scripts/claude-code-signal-hook
echo '{"event":"Notification","session_id":"demo"}' | ./scripts/claude-code-signal-hook
```

Supported Claude Code events include:

| Claude Code event | Signal behavior |
| --- | --- |
| `SessionStart` | Green idle |
| `UserPromptSubmit` | Working cycle |
| `PreToolUse` | Working cycle |
| `PostToolUse` | Working cycle |
| `PostToolUseFailure` | Red flashing |
| `Notification` | Yellow flashing |
| `PermissionRequest` | Red flashing |
| `Stop` | Green completion cue, then aggregate state without that session's normal work |
| `SessionEnd` | Session cleanup; if still tracked, brief green completion blink, then current aggregate state |

Claude Code 会通过 stdin 传入 JSON hook 数据，因此 wrapper 通常不需要额外参数：

```bash
echo '{"event":"PreToolUse","session_id":"demo"}' | ./scripts/claude-code-signal-hook
echo '{"event":"PermissionRequest","session_id":"demo"}' | ./scripts/claude-code-signal-hook
echo '{"event":"Notification","session_id":"demo"}' | ./scripts/claude-code-signal-hook
```

支持的 Claude Code 事件包括：

| Claude Code 事件 | 灯效行为 |
| --- | --- |
| `SessionStart` | 绿灯空闲 |
| `UserPromptSubmit` | 工作循环 |
| `PreToolUse` | 工作循环 |
| `PostToolUse` | 工作循环 |
| `PostToolUseFailure` | 红灯闪烁 |
| `Notification` | 黄灯闪烁 |
| `PermissionRequest` | 红灯闪烁 |
| `Stop` | 绿灯提示完成，然后恢复去掉该会话普通工作态后的聚合状态 |
| `SessionEnd` | 会话清理；如果该会话仍被跟踪，则绿灯短闪提示完成，然后恢复当前聚合状态 |

See [docs/LAMP_LANGUAGE.md](docs/LAMP_LANGUAGE.md) for a complete `~/.claude/settings.json` example.

完整 `~/.claude/settings.json` 示例见 [docs/LAMP_LANGUAGE.md](docs/LAMP_LANGUAGE.md)。

## OpenCode Integration / OpenCode 集成

OpenCode support is installed as a plugin file under `~/.config/opencode/plugins/signal-light.ts`:

```bash
./scripts/install-hooks --agent opencode -y
```

The generated plugin forwards supported OpenCode events into the same native runtime:

| OpenCode event | Signal behavior |
| --- | --- |
| `session.created` | Green idle |
| `session.idle` | Green completion cue, then aggregate state without that session's normal work |
| `session.status` | `idle`/`busy`/`retry` map to completion cue / working cycle / red flashing |
| `session.error` | Red flashing |
| `tool.execute.before` | Working cycle |
| `tool.execute.after` | Working cycle, or red flashing if the tool output reports a failure |
| `permission.asked` | Red flashing |
| `command.executed` | Working cycle |

OpenCode 会通过插件文件安装到 `~/.config/opencode/plugins/signal-light.ts`：

```bash
./scripts/install-hooks --agent opencode -y
```

生成的插件会把支持的 OpenCode 事件转发到同一个 native runtime：

| OpenCode 事件 | 灯效行为 |
| --- | --- |
| `session.created` | 绿灯空闲 |
| `session.idle` | 绿灯提示完成，然后恢复去掉该会话普通工作态后的聚合状态 |
| `session.status` | `idle`/`busy`/`retry` 分别映射为完成提示 / 工作循环 / 红灯闪烁 |
| `session.error` | 红灯闪烁 |
| `tool.execute.before` | 工作循环 |
| `tool.execute.after` | 工作循环；如果工具输出报错则切到红灯闪烁 |
| `permission.asked` | 红灯闪烁 |
| `command.executed` | 工作循环 |

完整插件示例见 [docs/LAMP_LANGUAGE.md](docs/LAMP_LANGUAGE.md)。

## Multi-Session Behavior / 多会话行为

The runtime stores the latest state for each agent session and shows the highest-priority aggregate on the physical light:

```text
red flashing > yellow flashing > working cycle > steady green
```

That means one session waiting for permission will stay red even if another session starts working. A normal `Stop` only clears non-urgent working state; it does not erase an existing red alert. The local server also removes sessions whose recorded owner process has exited, which keeps a killed local run from leaving the lamp stuck in an old working state.

When one tracked turn or session ends while other sessions are still running, the runtime briefly flashes green as a completion cue, then restores the current aggregate state. If all sessions have ended, it settles on steady green. If it remains idle, the light turns fully off after `SIGNAL_LIGHT_IDLE_SLEEP_SECONDS` (10 minutes by default). Red or yellow alerts are not interrupted by this completion cue.

运行时会记录每个 Agent 会话的最新状态，并把最高优先级状态显示到真实信号灯上：

```text
红灯闪烁 > 黄灯闪烁 > 工作循环 > 绿灯常亮
```

因此，一个会话正在等待权限时，即使另一个会话开始工作，红灯也不会被覆盖。普通 `Stop` 只会清掉非紧急的工作态，不会误清除已有红灯告警。

当某个已记录的会话结束、但其它会话还在运行时，运行时会让绿灯短暂闪烁，提示“有一个会话完成了”，然后恢复当前聚合状态。如果所有会话都结束了，最终会回到绿灯常亮。红灯或黄灯告警不会被这个完成提示打断。

## Project Status / 项目状态

This is a small, hackable hardware companion for AI-assisted development. It is designed to be easy to fork, rewire, and adapt:

- Swap MCP2221A for another GPIO backend.
- Add true PWM or LED strip drivers.
- Map other agent systems into the same lamp language.
- Build a nicer enclosure and put it on your desk.

这是一个小而可改的 AI 编程硬件伴侣项目。你可以很容易地 fork 并扩展它：

- 把 MCP2221A 换成其他 GPIO 后端。
- 增加真正的 PWM 或灯带驱动。
- 把更多 Agent 系统映射到同一套灯语。
- 做一个更漂亮的外壳，把它放到桌面上。

If your AI agent has become part of your workflow, give it a signal light.

如果 AI Agent 已经成了你的工作流的一部分，给它一盏真正的状态灯。

## License / 许可证

This project is licensed under the MIT License. See [LICENSE](LICENSE).

本项目使用 MIT 许可证开源。详见 [LICENSE](LICENSE)。
