import { type Status } from "./api.js";
import { type Input } from "./measure.js";
import { type Shell, watching } from "./shell.js";

export interface PointerInput {
  /** The listeners and the scripted run (?measure=1) call the same input functions. */
  input: Input;
  /** The last pointer position over the canvas, in client pixels; null once it left. */
  position: () => [number, number] | null;
  /** Forget the position (a new schedule was opened). */
  forget: () => void;
  /** Set the cursor from the frame's status: a crosshair for a link drag, `not-allowed` over a target it would refuse, a two-way arrow on a bar's finish edge. */
  sync: (s: Status) => void;
}

/**
 * The canvas's pointer and wheel listeners (the wheel one also on the "what moved" note), and the resize observer. `editing` says a cell is being edited;
 * `moved` tells the measurement a pointer move with the event's own time stamp.
 */
export function installPointer( // oxlint-disable-line max-lines-per-function -- existing, over cap
  shell: Shell,
  editing: () => boolean,
  moved: (at: number) => void,
): PointerInput {
  const { app, canvas, root } = shell;
  // Canvas-relative CSS pixels.
  const local = (e: MouseEvent): [number, number] => {
    const r = canvas.getBoundingClientRect();
    return [e.clientX - r.left, e.clientY - r.top];
  };

  // The listeners and the scripted run (?measure=1) call the same input functions.
  const input: Input = {
    wheel: (x, y, dx, dy, zoom) => app.wheel(x, y, dx, dy, zoom),
    down: (x, y, alt) => {
      canvas.classList.add("dragging");
      app.pointer_down(x, y, alt);
    },
    move: (x, y, at) => {
      app.pointer_move(x, y);
      if (at !== undefined) moved(at);
    },
    up: () => {
      canvas.classList.remove("dragging");
      app.pointer_up();
    },
    // A cancelled gesture (the browser took the pointer for a scroll or a
    // pinch) undoes a bar drag instead of committing its last move.
    cancel: () => {
      canvas.classList.remove("dragging");
      app.pointer_cancel();
    },
  };
  const onWheel = (e: WheelEvent): void => {
    e.preventDefault();
    const scale = e.deltaMode === 1 ? 16 : e.deltaMode === 2 ? canvas.clientHeight : 1;
    const [x, y] = local(e);
    // Pinch-zoom arrives as a ctrl wheel event.
    input.wheel(x, y, e.deltaX * scale, e.deltaY * scale, e.ctrlKey || e.metaKey);
  };
  canvas.addEventListener("wheel", onWheel, { passive: false });
  // The "what moved" note takes the pointer while it shows (`moved.css`) and lies over the canvas, so a wheel or a pinch
  // over it would reach the page instead (gs#549): it goes to the same handler, with the position still canvas-relative.
  root.querySelector<HTMLElement>("[data-part=moved-note]")?.addEventListener("wheel", onWheel, { passive: false });
  // The pointer that holds the press now, or the last one that did.
  let holder = -1;
  canvas.addEventListener("pointerdown", (e) => {
    if (e.button !== 0) return;
    if (editing()) return;
    // A press is down: the same pointer pressing again means its release was lost, so that press ends first; another pointer
    // (a second finger) is ignored, and so is the press it would have made (gantt#1152).
    if (shell.status().gesture !== "idle") {
      if (e.pointerId !== holder) return;
      input.cancel();
    }
    holder = e.pointerId;
    canvas.setPointerCapture(e.pointerId);
    input.down(...local(e), e.altKey);
  });
  // The event's own timeStamp, not the frame's: the drag response starts when the browser took the input.
  let pointer: [number, number] | null = null;
  canvas.addEventListener("pointermove", (e) => {
    pointer = [e.clientX, e.clientY];
    input.move(...local(e), e.timeStamp);
  });
  canvas.addEventListener("pointerup", () => input.up());
  canvas.addEventListener("pointercancel", () => input.cancel());
  canvas.addEventListener("pointerleave", () => {
    pointer = null;
    app.pointer_leave();
  });

  watching(
    shell,
    new ResizeObserver(() => {
      const r = canvas.getBoundingClientRect();
      app.resize(r.width, r.height, devicePixelRatio);
    }),
  ).observe(canvas);

  return {
    input,
    position: () => pointer,
    forget: () => {
      pointer = null;
    },
    sync: (s) => {
      canvas.classList.toggle("linking", s.link.drag !== null);
      canvas.classList.toggle("refused", s.link.drag?.refused != null);
      // A two-way arrow over a bar's finish edge while no press is down (a bar move or a link keeps its own cursor over another
      // bar's edge), and for the whole of an edge drag, even once the pointer has left the edge.
      canvas.classList.toggle(
        "resize",
        (s.gesture === "idle" && s.hover?.edge === true) || s.gesture === "resize-pressed" || s.gesture === "resize-dragging",
      );
    },
  };
}
