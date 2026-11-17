// Claim marks. A block that is not true yet (class "cloud", with data-claim naming the stories that make it true) gets a
// revision cloud, the scalloped outline a drafter draws around a pending change, and a tag with its stories. The path
// runs clockwise, so every arc bulges outward; it is redrawn when the box changes size. Inline claims are styled in CSS.
(() => {
  const NS = "http://www.w3.org/2000/svg";
  const STEP = 22;
  const INSET = 6;

  function edge(from, to, out) {
    const len = Math.hypot(to[0] - from[0], to[1] - from[1]);
    const n = Math.max(1, Math.round(len / STEP));
    const dx = (to[0] - from[0]) / n;
    const dy = (to[1] - from[1]) / n;
    const r = (len / n) * 0.62;
    for (let i = 1; i <= n; i++) {
      out.push(`A${r.toFixed(1)} ${r.toFixed(1)} 0 0 1 ${(from[0] + dx * i).toFixed(1)} ${(from[1] + dy * i).toFixed(1)}`);
    }
  }

  function draw(box, svg) {
    const w = box.clientWidth;
    const h = box.clientHeight;
    const a = [INSET, INSET];
    const b = [w - INSET, INSET];
    const c = [w - INSET, h - INSET];
    const d = [INSET, h - INSET];
    const parts = [`M${a[0]} ${a[1]}`];
    edge(a, b, parts);
    edge(b, c, parts);
    edge(c, d, parts);
    edge(d, a, parts);
    svg.setAttribute("viewBox", `0 0 ${w} ${h}`);
    svg.firstChild.setAttribute("d", parts.join(" ") + "Z");
  }

  for (const box of document.querySelectorAll(".cloud")) {
    const svg = document.createElementNS(NS, "svg");
    svg.setAttribute("class", "scallop");
    svg.setAttribute("aria-hidden", "true");
    svg.appendChild(document.createElementNS(NS, "path"));
    box.prepend(svg);
    const tag = document.createElement("span");
    tag.className = "claim-tag";
    tag.textContent = box.dataset.claim ?? "claim";
    tag.title = "Not true yet: the stories that make it true";
    box.append(tag);
    draw(box, svg);
    new ResizeObserver(() => draw(box, svg)).observe(box);
  }

  const open = document.querySelectorAll("[data-claim]").length;
  const note = document.getElementById("claims-open");
  if (note) note.textContent = `${open} claim${open === 1 ? "" : "s"} not true yet`;
})();
