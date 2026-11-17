// A chart's parts and their ids (gs#1083). The chart's markup (`chart.html`) names each part with `data-part`, and every
// lookup and style rule finds a part by that name inside the chart's root, never on the page. Ids are given at mount, per
// root, so a page with several charts has no id twice and an `aria-labelledby` names its own chart's element: the first chart
// on a page writes a part's id as its name (`timeline`), and each further chart puts its own prefix before it
// (`chart-2-timeline`). A destroyed chart gives its prefix back.

/** The selector for the part called `name`. */
export function part(name: string): string {
  return `[data-part=${name}]`;
}

/**
 * The part called `name` inside `root`, the chart's root (or, for the developer page's entry, the document), so a page with
 * two charts finds each chart's own parts.
 */
export function element<T extends HTMLElement>(root: ParentNode, name: string): T {
  const el = optional<T>(root, name);
  if (!el) throw new Error(`missing ${part(name)}`);
  return el;
}

/** An element of the app's chrome (the toolbar, the status line); the chart works when it is not there. */
export function optional<T extends HTMLElement>(root: ParentNode, name: string): T | null {
  return root.querySelector<T>(part(name));
}

/** The attributes that name other elements by id; in the markup they name parts. */
const REFERENCES = ["aria-labelledby", "aria-describedby", "aria-controls", "aria-owns", "aria-activedescendant", "for"];

/** The chart number of each mounted root (1 writes bare names, `n` writes `chart-n-`), and how many claims hold it. */
const numbers = new WeakMap<Node, { n: number; claims: number }>();
/** The chart numbers in use on the page. */
const taken = new Set<number>();

/**
 * Give the chart in `root` its id prefix, the first free number's (the root's own if it has one), and return the function
 * that gives this claim back; the number is free once every claim on the root is given back. A switch to WebGL2 holds a
 * claim while it takes the old start down and puts the new one up, so the chart keeps its number and its ids.
 */
export function claimIds(root: Node): () => void {
  let held = numbers.get(root);
  if (held === undefined) {
    let n = 1;
    while (taken.has(n)) n += 1;
    taken.add(n);
    held = { n, claims: 0 };
    numbers.set(root, held);
  }
  held.claims += 1;
  let given = false;
  return () => {
    const now = numbers.get(root);
    if (given || now === undefined) return;
    given = true;
    now.claims -= 1;
    if (now.claims > 0) return;
    taken.delete(now.n);
    numbers.delete(root);
  };
}

/**
 * Give each part in `within` (a piece of the chart's markup, or the root) its id, its name after the root's prefix,
 * and write each id reference in it (`aria-labelledby`, `for` and the rest) the same way.
 */
export function nameParts(root: Node, within: ParentNode): void {
  const n = numbers.get(root)?.n ?? 1;
  const prefix = n === 1 ? "" : `chart-${n}-`;
  // The root is the host's element: its own id and references are left as the host wrote them.
  const all = [...within.querySelectorAll("*")];
  if (within !== root && within instanceof Element) all.unshift(within);
  for (const el of all) {
    if (el instanceof HTMLElement && el.dataset.part !== undefined) el.id = prefix + el.dataset.part;
    for (const name of REFERENCES) {
      const names = el.getAttribute(name);
      if (names !== null) el.setAttribute(name, names.split(/\s+/).filter((ref) => ref !== "").map((ref) => prefix + ref).join(" "));
    }
  }
}
