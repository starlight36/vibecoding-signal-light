import type { Plugin } from "@opencode-ai/plugin"
import { spawn } from "child_process"

const WRAPPER_SCRIPT = __HOOK_SCRIPT_PATH_JSON__

const EVENT_SIGNALS: Record<string, string> = {
  "session.created": "session_start",
  "session.idle": "turn_end",
  "session.error": "blocked",
  "permission.asked": "permission",
  "command.executed": "working",
}

const SESSION_STATUS_SIGNALS: Record<string, string> = {
  idle: "turn_end",
  busy: "working",
  retry: "blocked",
}

const sessionIDOf = (properties: any): string | undefined =>
  properties?.sessionID ?? properties?.info?.id

const SignalLightPlugin: Plugin = async ({ directory }) => {
  const fire = (eventName: string, signal: string, sessionID?: string): Promise<void> => {
    return new Promise((resolve) => {
      let child: ReturnType<typeof spawn>
      try {
        const payload = JSON.stringify({
          hook_event_name: eventName,
          signal,
          session_id: sessionID,
          cwd: directory,
        })
        child = spawn(WRAPPER_SCRIPT, [], { stdio: ["pipe", "ignore", "ignore"] })
        child.stdin.end(payload + "\n")
      } catch {
        resolve()
        return
      }
      child.on("error", () => resolve())
      child.stdin.on("error", () => resolve())
      child.on("close", () => resolve())
    })
  }

  return {
    event: async ({ event }) => {
      if (event.type === "session.status") {
        const statusType = (event.properties?.status as { type?: string } | undefined)?.type
        const signal = statusType ? SESSION_STATUS_SIGNALS[statusType] : undefined
        if (signal) await fire(event.type, signal, sessionIDOf(event.properties))
        return
      }
      const signalName = EVENT_SIGNALS[event.type]
      if (!signalName) return
      await fire(event.type, signalName, sessionIDOf(event.properties))
    },
    "tool.execute.before": async (input) => {
      await fire("tool.execute.before", "working", input.sessionID)
    },
    "tool.execute.after": async (input, output) => {
      const failed = /error|failed|exception/i.test(output.output ?? "")
      await fire("tool.execute.after", failed ? "blocked" : "tool_done", input.sessionID)
    },
  }
}

export default SignalLightPlugin
