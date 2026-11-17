// The engine's stress fixture, `engine::generate::stress(seed, n)`, as benchmark records: the
// same tasks, the same links and the same task ids, drawn in the same order from the same
// SplitMix64 stream. Only what the benchmark dataset carries is kept: a task's duration (in 8-hour
// days) and each link's type. The calendars, lags and start-no-earlier-than dates are drawn, so
// the stream stays in step, and then dropped. This shape is ours, not the comparison harness's.
//
// `crates/engine/src/generate.rs` is the authority: a change there that moves a draw must move the
// same draw here, and the hash pinned in `dataset.test.ts` then changes.

const MASK = (1n << 64n) - 1n;

/** SplitMix64, as `engine::generate::Rng`. */
export class Rng {
  #state: bigint;
  constructor(seed: bigint) {
    this.#state = seed & MASK;
  }
  nextU64(): bigint {
    this.#state = (this.#state + 0x9e3779b97f4a7c15n) & MASK;
    let z = this.#state;
    z = ((z ^ (z >> 30n)) * 0xbf58476d1ce4e5b9n) & MASK;
    z = ((z ^ (z >> 27n)) * 0x94d049bb133111ebn) & MASK;
    return z ^ (z >> 31n);
  }
  /** Uniform in `lo..=hi`. */
  range(lo: number, hi: number): number {
    const span = BigInt(hi - lo) + 1n;
    return lo + Number(this.nextU64() % span);
  }
  /** True with probability `p`. */
  chance(p: number): boolean {
    return Number(this.nextU64() >> 11n) / 2 ** 53 < p;
  }
}

/** Link types in the engine's order. */
export type Kind = "FS" | "SS" | "FF" | "SF";

export interface StressTask {
  /** Working minutes. */
  minutes: number;
  head: boolean;
}

export interface StressLink {
  pred: number;
  succ: number;
  kind: Kind;
}

/** Tasks in id order (index = the engine's `TaskId`) and links in the engine's order. */
export interface Stress {
  tasks: StressTask[];
  links: StressLink[];
  head: number;
}

/** The draws of `add_calendars` without working exceptions: two holiday lists of 400 days each. */
function skipCalendars(rng: Rng): void {
  for (let i = 0; i < 2 * 40 * 10; i++) rng.range(0, 30);
}

/** `draft(rng, n, cals, dense = true, Shape::Chain)`, keeping durations and links. */
function draft(rng: Rng, n: number) {
  const minutes: number[] = [];
  const deps: [number, number, Kind][] = [];
  const streamStarts: number[] = [];
  const streams = Math.max(Math.floor(n / 2500), 1);
  const perStream = Math.ceil(n / streams);
  const phaseStarts: number[] = [];
  let i = 0;
  for (let s = 0; s < streams; s++) {
    const streamEnd = Math.min(i + perStream, n);
    const phases: [number, number][] = [];
    streamStarts.push(i);
    while (i < streamEnd) {
      const len = rng.range(20, 120);
      const start = i;
      const end = Math.min(i + len, streamEnd);
      phaseStarts.push(start);
      for (let t = start; t < end; t++) {
        rng.range(0, 99); // the calendar
        minutes.push(rng.chance(0.03) ? 0 : rng.range(1, 40) * 120);
        if (rng.chance(0.02)) rng.range(0, 3 * 365); // the start-no-earlier-than day
        const chosen: number[] = [];
        const link = (p: number) => {
          if (chosen.includes(p)) return;
          chosen.push(p);
          const k = rng.range(0, 99);
          const kind: Kind = k < 80 ? "FS" : k < 90 ? "SS" : k < 97 ? "FF" : "SF";
          if (rng.chance(0.2)) rng.range(-8, 16); // the lag
          deps.push([p, t, kind]);
        };
        if (t === start) {
          const last = phases.at(-1);
          if (last) link(rng.range(Math.floor((last[0] + last[1]) / 2), last[1] - 1));
        } else {
          const lo = Math.max(start, t - 8);
          link(rng.range(lo, t - 1));
          if (rng.chance(0.45)) link(rng.range(lo, t - 1));
        }
        if (t > 0 && rng.chance(0.03)) link(rng.range(0, t - 1));
        if (phaseStarts.length > 1) {
          for (let r = 0; r < 2; r++) {
            const ph = rng.range(0, phaseStarts.length - 2);
            link(rng.range(phaseStarts[ph], phaseStarts[ph + 1] - 1));
          }
        }
      }
      phases.push([start, end]);
      i = end;
    }
  }
  return { minutes, deps, streamStarts };
}

/** `engine::generate::stress(seed, n)`: `n` tasks, the head included. */
export function stress(seed: bigint, n: number): Stress {
  if (n < 2) throw new Error("the stress shape needs at least two tasks");
  const rng = new Rng(seed);
  skipCalendars(rng);
  const d = draft(rng, n - 1);
  const head = d.minutes.length;
  d.minutes.push(480);
  for (const st of d.streamStarts) d.deps.push([head, st, "FS"]);
  // A seeded Fisher-Yates permutation: generation index to task id.
  const perm = Array.from({ length: n }, (_, g) => g);
  for (let i = n - 1; i >= 1; i--) {
    const j = rng.range(0, i);
    [perm[i], perm[j]] = [perm[j], perm[i]];
  }
  const tasks: StressTask[] = new Array(n);
  perm.forEach((id, g) => (tasks[id] = { minutes: d.minutes[g], head: g === head }));
  const links = d.deps.map(([p, t, kind]) => ({ pred: perm[p], succ: perm[t], kind }));
  return { tasks, links, head: perm[head] };
}
