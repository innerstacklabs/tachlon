// The one browser launch every CI Chromium start goes through (gantt#1656; the benchmark driver's retry from gantt#1642, moved here so `scripts/`,
// `examples/` and `demos/bench/` share it).

/** Start the browser. A Chromium binary another process is still writing fails with ETXTBSY (gantt#1642): retry that, 3 times, `gapMs` apart. */
export async function launchRetrying<T>(launcher: () => Promise<T>, gapMs = 1000): Promise<T> {
  for (let retries = 0; ; retries++) {
    try {
      return await launcher();
    } catch (e) {
      if (retries >= 3 || !(/ETXTBSY/.test(String(e)) || (e as { code?: string })?.code === "ETXTBSY")) throw e;
      await new Promise((r) => setTimeout(r, gapMs));
    }
  }
}
