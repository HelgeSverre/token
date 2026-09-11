/**
 * Small, dependency-free helpers for Token's local automation socket.
 *
 * This module deliberately requires an explicit Unix socket path. It does not
 * discover, or silently connect to, the editor a developer happens to have
 * running. Native smoke scripts should normally use `startIsolatedToken`,
 * which creates a private fixture, configuration directory, and socket under
 * `target/verification/`.
 */
import { spawn } from "node:child_process";
import { mkdir, mkdtemp, readFile, writeFile } from "node:fs/promises";
import net from "node:net";
import path from "node:path";
import { setTimeout as delay } from "node:timers/promises";

export const DEFAULT_TIMEOUT_MS = 5_000;
export const DEFAULT_POLL_INTERVAL_MS = 100;
const MAX_INPUT_EVENTS = 64;

/** An automation failure with the reply, when Token supplied one. */
export class TokenAutomationError extends Error {
  constructor(message, { response, cause } = {}) {
    super(message, { cause });
    this.name = "TokenAutomationError";
    this.response = response;
  }
}

function requiredString(value, name) {
  if (typeof value !== "string" || value.length === 0)
    throw new TypeError(`${name} must be a non-empty string`);
  return value;
}

function finite(value, name) {
  if (!Number.isFinite(value)) throw new TypeError(`${name} must be finite`);
  return value;
}

function boundedPositive(value, name) {
  if (!Number.isFinite(value) || value <= 0)
    throw new TypeError(`${name} must be a positive number`);
  return value;
}

function waitFor(promise, timeoutMs, label) {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      reject(
        new TokenAutomationError(`Timed out after ${timeoutMs} ms: ${label}`),
      );
    }, timeoutMs);
    Promise.resolve(promise).then(
      (result) => {
        clearTimeout(timer);
        resolve(result);
      },
      (error) => {
        clearTimeout(timer);
        reject(error);
      },
    );
  });
}

/**
 * Poll `check` until it returns a truthy value, returning that value.
 *
 * Polling is bounded by default; use a label that makes a CI failure useful.
 */
export async function until(
  check,
  {
    label = "condition",
    timeoutMs = 10_000,
    intervalMs = DEFAULT_POLL_INTERVAL_MS,
  } = {},
) {
  if (typeof check !== "function")
    throw new TypeError("check must be a function");
  boundedPositive(timeoutMs, "timeoutMs");
  boundedPositive(intervalMs, "intervalMs");

  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const remaining = Math.max(1, deadline - Date.now());
    const result = await waitFor(
      Promise.resolve().then(check),
      remaining,
      label,
    );
    if (result) return result;
    await delay(Math.min(intervalMs, Math.max(1, deadline - Date.now())));
  }
  throw new TokenAutomationError(`Timed out after ${timeoutMs} ms: ${label}`);
}

/** Convenient constructors for Token's native-window input events. */
export const inputEvent = Object.freeze({
  close: () => ({ kind: "close" }),
  focus: (focused) => ({ kind: "focus", focused: Boolean(focused) }),
  move: (x, y) => ({
    kind: "pointer_move",
    x: finite(x, "x"),
    y: finite(y, "y"),
  }),
  button: (pressed, button = "left") => {
    if (!new Set(["left", "right", "middle"]).has(button))
      throw new TypeError("button must be left, right, or middle");
    return { kind: "pointer_button", button, pressed: Boolean(pressed) };
  },
  wheel: (x, y, unit = "pixels") => {
    if (!new Set(["pixels", "lines"]).has(unit))
      throw new TypeError("wheel unit must be pixels or lines");
    return {
      kind: "wheel",
      x: finite(x, "wheel x"),
      y: finite(y, "wheel y"),
      unit,
    };
  },
});

/** Return the center of Token's `[x, y, width, height]` geometry rectangle. */
export function rectCenter(rect) {
  if (!Array.isArray(rect) || rect.length !== 4)
    throw new TypeError("rect must be [x, y, width, height]");
  const [x, y, width, height] = rect.map((value, index) =>
    finite(value, `rect[${index}]`),
  );
  return [x + width / 2, y + height / 2];
}

function socketRequest(socketPath, body, timeoutMs) {
  let payload;
  try {
    payload = `${JSON.stringify(body)}\n`;
  } catch (error) {
    return Promise.reject(
      new TokenAutomationError(
        "Automation request could not be encoded as JSON",
        {
          cause: error,
        },
      ),
    );
  }

  return new Promise((resolve, reject) => {
    const client = net.createConnection(socketPath);
    client.setEncoding("utf8");
    let data = "";
    let settled = false;
    let requestTimer;
    const finish = (callback, value) => {
      if (settled) return;
      settled = true;
      clearTimeout(requestTimer);
      client.destroy();
      callback(value);
    };

    requestTimer = setTimeout(
      () =>
        finish(
          reject,
          new TokenAutomationError(
            `Automation request timed out after ${timeoutMs} ms`,
          ),
        ),
      timeoutMs,
    );
    client.on("error", (error) => finish(reject, error));
    client.on("connect", () => client.write(payload));
    client.on("data", (chunk) => {
      data += chunk;
      const newline = data.indexOf("\n");
      if (newline === -1) return;
      try {
        finish(resolve, JSON.parse(data.slice(0, newline)));
      } catch (error) {
        finish(
          reject,
          new TokenAutomationError("Automation replied with invalid JSON", {
            cause: error,
          }),
        );
      }
    });
    client.on("end", () => {
      if (!settled)
        finish(
          reject,
          new TokenAutomationError(
            "Automation closed without a complete reply",
          ),
        );
    });
  });
}

function requireReplyPart(response, key, requestType) {
  if (response[key] === null || response[key] === undefined)
    throw new TokenAutomationError(
      `Token ${requestType} reply did not include ${key}`,
      { response },
    );
  return response[key];
}

/**
 * Create a client for one explicit Unix-domain socket.
 *
 * `request` returns the complete protocol reply. The other methods unwrap the
 * relevant reply field, so `state`, `document`, and `actions` retain their
 * distinct response shapes.
 */
export function createTokenClient({
  socketPath,
  timeoutMs = DEFAULT_TIMEOUT_MS,
}) {
  requiredString(socketPath, "socketPath");
  boundedPositive(timeoutMs, "timeoutMs");

  async function request(
    body,
    { timeoutMs: requestTimeoutMs = timeoutMs } = {},
  ) {
    if (!body || typeof body !== "object")
      throw new TypeError("automation request must be an object");
    boundedPositive(requestTimeoutMs, "request timeoutMs");
    const response = await socketRequest(socketPath, body, requestTimeoutMs);
    if (!response?.ok)
      throw new TokenAutomationError(
        response?.message || "Token rejected the automation request",
        { response },
      );
    return response;
  }

  const stateFor = async (body, options) =>
    requireReplyPart(await request(body, options), "state", body.type);
  return Object.freeze({
    socketPath,
    request,
    state: () => stateFor({ type: "state" }),
    document: async () =>
      requireReplyPart(
        await request({ type: "document" }),
        "document",
        "document",
      ),
    actions: async () =>
      requireReplyPart(
        await request({ type: "actions" }),
        "actions",
        "actions",
      ),
    insertText: (text) => stateFor({ type: "insert_text", text: String(text) }),
    setCursor: (line, column) => stateFor({ type: "set_cursor", line, column }),
    setSelection: (anchorLine, anchorColumn, headLine, headColumn) =>
      stateFor({
        type: "set_selection",
        anchor_line: anchorLine,
        anchor_column: anchorColumn,
        head_line: headLine,
        head_column: headColumn,
      }),
    action: (name) => stateFor({ type: "execute_action", name }),
    scroll: (lines) => stateFor({ type: "scroll", lines }),
    input: (events) => {
      if (
        !Array.isArray(events) ||
        events.length === 0 ||
        events.length > MAX_INPUT_EVENTS
      )
        throw new RangeError(
          `input events must contain 1-${MAX_INPUT_EVENTS} events`,
        );
      return stateFor({ type: "input", events });
    },
    setOverlayInput: (text) =>
      stateFor({ type: "set_overlay_input", text: String(text) }),
    openPaths: (paths, { wait = false, timeoutMs: openTimeoutMs } = {}) =>
      stateFor(
        { type: "open_paths", paths, wait },
        // Token may keep `wait: true` open indefinitely; this client remains
        // bounded unless the caller deliberately supplies a longer timeout.
        openTimeoutMs === undefined ? undefined : { timeoutMs: openTimeoutMs },
      ),
    profileFrames: (frames) => request({ type: "profile_frames", frames }),
    profileSyntax: (text) =>
      request({ type: "profile_syntax", text: String(text) }),
  });
}

/**
 * Keep a pointer position while dispatching atomic move/button/wheel batches.
 * This mirrors the native input contract without touching the system cursor.
 */
export function createPointer(client) {
  if (!client?.input)
    throw new TypeError("client must be a Token automation client");
  let position;
  const current = () => {
    if (!position)
      throw new TokenAutomationError(
        "move the pointer before button or wheel input",
      );
    return position;
  };
  return Object.freeze({
    move(x, y) {
      position = inputEvent.move(x, y);
      return client.input([position]);
    },
    button(pressed, button = "left") {
      return client.input([current(), inputEvent.button(pressed, button)]);
    },
    wheel(x, y, unit = "pixels") {
      return client.input([current(), inputEvent.wheel(x, y, unit)]);
    },
    async dragTo(x, y, button = "left") {
      await this.button(true, button);
      try {
        position = inputEvent.move(x, y);
        return await client.input([position]);
      } finally {
        await this.button(false, button);
      }
    },
  });
}

function fixturePath(fixture, relativePath) {
  requiredString(relativePath, "fixture file path");
  if (path.isAbsolute(relativePath))
    throw new TypeError("fixture file paths must be relative");
  const fixtureRoot = path.resolve(fixture);
  const resolved = path.resolve(fixtureRoot, relativePath);
  if (!resolved.startsWith(`${fixtureRoot}${path.sep}`))
    throw new TypeError(
      "fixture file paths must stay inside the fixture directory",
    );
  return resolved;
}

function childExit(child) {
  return new Promise((resolve) => {
    child.once("error", (error) => resolve({ error }));
    child.once("exit", (code, signal) => resolve({ code, signal }));
  });
}

/**
 * Start one private Token window for a Unix-native script.
 *
 * The helper never writes the user's config: it sets `XDG_CONFIG_HOME` to an
 * isolated directory and pins Token to a short socket under the fixture. It is
 * deliberately Unix-only because its endpoint is a Unix-domain socket.
 */
export async function startIsolatedToken({
  root,
  binary,
  name = "automation",
  configYaml = "",
  files = {},
  openFiles = [],
  args = [],
  env = {},
}) {
  requiredString(root, "root");
  requiredString(binary, "binary");
  if (
    typeof name !== "string" ||
    name.length === 0 ||
    name === "." ||
    name === ".." ||
    path.basename(name) !== name
  )
    throw new TypeError("name must be a single directory name");
  if (process.platform === "win32")
    throw new TokenAutomationError(
      "startIsolatedToken currently supports Unix-domain sockets only; use Token's CLI or MCP bridge for Windows automation",
    );

  const rootPath = path.resolve(root);
  const binaryPath = path.resolve(rootPath, binary);
  const base = path.join(rootPath, "target", "verification", name);
  await mkdir(base, { recursive: true });
  const fixture = await mkdtemp(path.join(base, "run-"));
  const socketPath = path.join(fixture, "s");
  const config = path.join(fixture, "config");
  await mkdir(path.join(config, "token-editor"), { recursive: true });
  await writeFile(path.join(config, "token-editor", "config.yaml"), configYaml);

  const writeFixture = async (relativePath, contents) => {
    const file = fixturePath(fixture, relativePath);
    await mkdir(path.dirname(file), { recursive: true });
    await writeFile(file, contents);
    return file;
  };
  for (const [relativePath, contents] of Object.entries(files))
    await writeFixture(relativePath, contents);

  let stderr = "";
  const child = spawn(
    binaryPath,
    [
      "--foreground",
      "--new-window",
      fixture,
      ...openFiles.map((relativePath) => fixturePath(fixture, relativePath)),
      ...args,
    ],
    {
      cwd: rootPath,
      env: {
        ...process.env,
        ...env,
        XDG_CONFIG_HOME: config,
        TOKEN_AUTOMATION_SOCKET: socketPath,
      },
      stdio: ["ignore", "ignore", "pipe"],
    },
  );
  child.stderr.on("data", (chunk) => {
    stderr = (stderr + chunk).slice(-16_000);
  });
  const exited = childExit(child);
  const client = createTokenClient({ socketPath });

  return Object.freeze({
    root: rootPath,
    fixture,
    socketPath,
    config,
    child,
    client,
    get stderr() {
      return stderr;
    },
    readFixture: (relativePath, encoding = "utf8") =>
      readFile(fixturePath(fixture, relativePath), encoding),
    writeFixture,
    async waitForStartup({ timeoutMs = 60_000, ready } = {}) {
      const isReady = ready ?? ((state) => state.editor_geometry);
      return until(
        async () => {
          if (child.exitCode !== null)
            throw new TokenAutomationError(
              `Token exited during startup: ${stderr}`,
            );
          try {
            const state = await client.state();
            return (await isReady(state)) ? state : false;
          } catch (error) {
            if (error?.code === "ENOENT" || error?.code === "ECONNREFUSED")
              return false;
            throw error;
          }
        },
        { label: "native Token window startup", timeoutMs },
      );
    },
    async stop({ timeoutMs = 5_000 } = {}) {
      try {
        await client.action("Quit");
      } catch {
        // The process may have failed during launch or already quit.
      }
      const result = await waitFor(exited, timeoutMs, "Token to quit").catch(
        (error) => {
          child.kill();
          throw error;
        },
      );
      if (result.error || result.code !== 0)
        throw new TokenAutomationError(
          `Token exited abnormally: ${result.error?.message || JSON.stringify(result)}`,
          { cause: result.error },
        );
      return result;
    },
  });
}
