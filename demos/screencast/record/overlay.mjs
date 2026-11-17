// What the recording draws over the page: the caption label, the ring, the cursor dot and the hidden metrics overlay.
// The page side lives in one init script and is reached through window.scOverlay, so a step never touches the DOM.

export const FADE_MS = 150;

export async function installOverlay(context) {
  await context.addInitScript((fadeMs) => {
    const install = () => {
      // A rounded dark label over the lower third of the timeline (the table beside it is not covered).
      // The text sits on the chart, 30 px so it still reads in the 560 px GIF.
      const caption = document.createElement("div");
      caption.id = "sc-caption";
      Object.assign(caption.style, {
        position: "fixed", left: "0", top: "0", transform: "translate(-50%, -50%)", textAlign: "center",
        zIndex: "2147483647", pointerEvents: "none", color: "white", boxSizing: "border-box",
        background: "rgba(15, 20, 30, 0.94)", font: "600 30px/1.25 system-ui, sans-serif",
        padding: "12px 22px", borderRadius: "16px", display: "none", opacity: "0",
        transition: `opacity ${fadeMs}ms ease`,
      });
      const place = () => {
        const box = document.getElementById("timeline")?.getBoundingClientRect();
        if (!box) throw new Error("#timeline not found, so the caption has nowhere to sit");
        caption.style.maxWidth = `${Math.round(box.width * 0.9)}px`;
        // Centre of the lower third, moved up when a wrapped caption would reach past the timeline's bottom edge.
        const half = caption.offsetHeight / 2;
        caption.style.left = `${box.left + box.width / 2}px`;
        caption.style.top = `${Math.min(box.top + (box.height * 5) / 6, box.bottom - half - 8)}px`;
      };
      const fade = (opacity) => new Promise((done) => {
        caption.style.opacity = opacity;
        setTimeout(done, fadeMs + 20);
      });
      let base = "";
      window.scOverlay = {
        // A caption already showing fades out first; the new one fades in while the step starts.
        async show(text) {
          if (caption.style.display !== "none" && caption.style.opacity === "1") await fade("0");
          if (!text) {
            caption.style.display = "none";
            base = "";
            return;
          }
          base = text;
          caption.textContent = text;
          caption.style.display = "block";
          place();
          caption.getBoundingClientRect(); // commit opacity 0 so the fade starts from it
          caption.style.opacity = "1";
        },
        // The caption gains its measured ending in place, without a fade.
        extend(ending) {
          if (caption.style.display === "none") throw new Error("extend: no caption is showing");
          caption.textContent = `${base} ${ending}`;
          place();
        },
        // Circle a page element for a moment; resolves when the last pulse has finished.
        async ring(selector, { ms = 1600, pulses = 2 } = {}) {
          const target = document.querySelector(selector);
          if (!target) throw new Error(`ring: nothing matches ${selector}`);
          const box = target.getBoundingClientRect();
          if (box.width === 0 || box.height === 0) throw new Error(`ring: ${selector} has no box`);
          const ring = document.createElement("div");
          Object.assign(ring.style, {
            position: "fixed", left: `${box.left - 10}px`, top: `${box.top - 6}px`,
            width: `${box.width + 20}px`, height: `${box.height + 12}px`, boxSizing: "border-box",
            border: "4px solid #ffd23f", borderRadius: "999px", boxShadow: "0 0 0 2px rgba(15, 20, 30, 0.7)",
            pointerEvents: "none", zIndex: "2147483646", opacity: "0",
          });
          document.body.append(ring);
          try {
            await ring.animate([
              { opacity: 0, transform: "scale(1.5)" },
              { opacity: 1, transform: "scale(1)", offset: 0.45 },
              { opacity: 1, transform: "scale(1)", offset: 0.75 },
              { opacity: 0, transform: "scale(1)" },
            ], { duration: ms / pulses, iterations: pulses, easing: "ease-out" }).finished;
          } finally {
            ring.remove();
          }
        },
      };
      // The metrics overlay prints frame times that change every frame (the video would never hold still, and
      // a screen recording inflates the numbers); the README says those numbers are for watching, not results.
      const hide = document.createElement("style");
      hide.textContent = "#overlay { display: none !important; }";
      document.head.append(hide);
      const cursor = document.createElement("div");
      cursor.id = "sc-cursor";
      Object.assign(cursor.style, {
        position: "fixed", width: "16px", height: "16px", border: "2px solid white",
        outline: "1px solid #18202b", borderRadius: "50%", boxSizing: "border-box",
        transform: "translate(-50%, -50%)", zIndex: "2147483647", pointerEvents: "none",
      });
      const move = (event) => {
        cursor.style.left = `${event.clientX}px`;
        cursor.style.top = `${event.clientY}px`;
        cursor.style.background = event.buttons ? "rgba(255, 255, 255, 0.75)" : "transparent";
      };
      for (const type of ["mousemove", "mousedown", "mouseup"]) window.addEventListener(type, move, { capture: true, passive: true });
      document.body.append(caption, cursor);
    };
    if (document.readyState === "loading") window.addEventListener("DOMContentLoaded", install, { once: true });
    else install();
  }, FADE_MS);
}

/** For a still: dismiss the hosted first-screen line (a DOM click, so the pointer stays put), hide the caption and the cursor ring, wait two frames. */
export async function hideForShot(page) {
  await page.evaluate(() => {
    document.querySelector("#first-screen-line button")?.click();
    for (const id of ["sc-caption", "sc-cursor"]) { const el = document.getElementById(id); if (el) el.style.visibility = "hidden"; }
  });
  await page.evaluate(() => new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done))));
}

/** Undo `hideForShot`'s hiding (the dismissed line stays dismissed). */
export const showAfterShot = (page) => page.evaluate(() => {
  for (const id of ["sc-caption", "sc-cursor"]) { const el = document.getElementById(id); if (el) el.style.visibility = ""; }
});
