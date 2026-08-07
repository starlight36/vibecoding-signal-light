import type { Plugin } from "@opencode-ai/plugin"

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

const SignalLightPlugin: Plugin = async ({ $, directory }) => {
  const fire = (eventName: string, signal: string, sessionID?: string) => {
    const payload = {
      hook_event_name: eventName,
      signal,
      session_id: sessionID,
      cwd: directory,
    }
    return $`printf %s ${JSON.stringify(payload)} | ${WRAPPER_SCRIPT}`
      .quiet()
      .catch(() => {})
  }

  return {
    event: async ({ event }) => {
      if (event.type === "session.status") {
        const statusType = (event.properties?.status as { type?: string } | undefined)?.type
        const signal = statusType ? SESSION_STATUS_SIGNALS[statusType] : undefined
        if (signal) await fire(event.type, signal, event.properties?.sessionID)
        return
      }
      const signalName = EVENT_SIGNALS[event.type]
      if (!signalName) return
      await fire(event.type, signalName, event.properties?.sessionID)
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
