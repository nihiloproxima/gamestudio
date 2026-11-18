// Reports this OpenCode session's status to Gamestudio, through `studio hook opencode`. Written to the config
// directory Gamestudio gives its own OpenCode sessions, and only those: the app's session id comes from the environment.
import { execFile } from "node:child_process";

const STUDIO = __STUDIO_EXE__;
const session = process.env.STUDIO_SESSION;

const running = new Set();
const waiting = new Map();
let lastText = "";
let last = "";

function report(status, message) {
  const key = `${status}\u0000${message ?? ""}`;
  if (key === last) return;
  last = key;
  execFile(STUDIO, ["hook", "opencode", JSON.stringify({ status, message })], () => {});
}

function current() {
  if (waiting.size) return ["waiting", [...waiting.values()][0]];
  if (running.size) return ["working", undefined];
  return ["idle", lastText];
}

function handle(event) {
  const data = event.data ?? {};
  switch (event.type) {
    case "session.execution.started":
      running.add(data.sessionID);
      break;
    case "session.execution.succeeded":
    case "session.execution.failed":
    case "session.execution.interrupted":
      running.delete(data.sessionID);
      break;
    case "session.text.ended":
      lastText = data.text ?? lastText;
      break;
    case "permission.asked":
      waiting.set(data.id, data.message ?? `Permission : ${data.action}`);
      break;
    case "permission.replied":
      waiting.delete(data.requestID);
      break;
    case "form.created":
      waiting.set(data.form?.id, data.form?.title ?? "Une question attend ta réponse");
      break;
    case "form.replied":
    case "form.cancelled":
      waiting.delete(data.id);
      break;
    default:
      return;
  }
  report(...current());
}

export default {
  id: "gamestudio",
  setup(ctx) {
    if (!session) return;
    (async () => {
      for await (const event of ctx.event.subscribe()) handle(event);
    })().catch(() => {});
  },
};
