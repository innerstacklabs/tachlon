// Helpers of the scripted measurement that touch the machine, not the page: option parsing, the browser
// binary, a free port and stopping the preview server. Used by `demos/50k/measure.mjs`.
import { readdirSync } from "node:fs";
import { createServer } from "node:net";
import { homedir } from "node:os";
import { join } from "node:path";

export function parseParam(arg) {
  const value = arg ?? "";
  const separator = value.indexOf("=");
  return separator < 0 ? [value, "1"] : [value.slice(0, separator), value.slice(separator + 1)];
}


export function newestChromium(names) {
  let newest;
  let newestVersion = -1;
  for (const name of names) {
    const match = /^chromium-(\d+)$/.exec(name);
    const version = match ? Number(match[1]) : -1;
    if (version > newestVersion) {
      newest = name;
      newestVersion = version;
    }
  }
  return newest;
}


export function playwrightChromium() {
  const base = join(homedir(), ".cache", "ms-playwright");
  const dir = newestChromium(readdirSync(base));
  if (!dir) throw new Error(`no Chromium under ${base}; pass --chrome`);
  return join(base, dir, "chrome-linux64", "chrome");
}


// Another preview may already hold 4173, and answering on it would measure the wrong build.
export function freePort() {
  return new Promise((done, fail) => {
    const probe = createServer();
    probe.once("error", fail);
    probe.listen(0, "127.0.0.1", () => {
      const { port } = probe.address();
      probe.close(() => done(port));
    });
  });
}


async function answers(url) {
  try {
    await fetch(url);
    return true;
  } catch {
    return false;
  }
}

// `pnpm preview` starts `sh`, which starts `node vite.js`; the node process holds the port. The child leads its
// own process group, so signalling the group (a negative pid) reaches all three, where `child.kill()` reaches only pnpm.
export async function stopGroup(child, url, options = {}) {
  const { graceMs = 5000 } = options;
  try {
    process.kill(-child.pid, "SIGTERM");
  } catch {
    // the group is already gone
  }
  const groupIsAlive = () => {
    try {
      process.kill(-child.pid, 0);
      return true;
    } catch (err) {
      if (err?.code === "ESRCH") return false;
      throw err;
    }
  };
  const deadline = Date.now() + graceMs;
  let alive = groupIsAlive();
  while (alive && Date.now() < deadline) {
    await new Promise((r) => setTimeout(r, 100));
    alive = groupIsAlive();
  }
  if (alive) {
    try {
      process.kill(-child.pid, "SIGKILL");
    } catch (err) {
      if (err?.code !== "ESRCH") throw err;
    }
    const killDeadline = Date.now() + 2000;
    while (groupIsAlive() && Date.now() < killDeadline) await new Promise((r) => setTimeout(r, 100));
  }
  for (let i = 0; i < 20; i++) {
    if (!(await answers(url))) return;
    await new Promise((r) => setTimeout(r, 250));
  }
  throw new Error(`the preview server still answers on ${url} after it was stopped; kill its process group ${child.pid} by hand`);
}
