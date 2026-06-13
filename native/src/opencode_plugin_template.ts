import type { Plugin } from "@opencode-ai/plugin"

const WRAPPER_SCRIPT = __HOOK_SCRIPT_PATH_JSON__

const EVENT_SIGNALS: Record<string, string> = {
  "session.created": "session_start",
  "session.idle": "turn_end",
  "session.error": "blocked",
  "tool.execute.before": "working",
  "tool.execute.after": "tool_done",
  "permission.asked": "permission",
  "command.executed": "working",
}

const SignalLightPlugin: Plugin = async ({ $, directory }) => {
  return {
    event: async ({ event }) => {
      const signalName = EVENT_SIGNALS[event.type]
      if (!signalName) return

      const payload = {
        hook_event_name: event.type,
        signal: signalName,
        session_id: event.properties?.sessionID,
        cwd: event.properties?.cwd || directory,
        owner_pid: event.properties?.pid,
      }

      await $`printf %s ${JSON.stringify(payload)} | ${WRAPPER_SCRIPT}`
        .quiet()
        .catch(() => {})
    },
  }
}

export default SignalLightPlugin
