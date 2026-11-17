import { readFileSync } from "node:fs";
import { inflateSync } from "node:zlib";
import { dpr } from "./knobs";

const tokens = readFileSync(new URL("../../src/styles/tokens.css", import.meta.url), "utf8");

/** CSS px, the time-axis band: read from the `--gantt-axis-h` token in tokens.css, the default `timeline::viewport::AXIS_HEIGHT` mirrors. */
export const AXIS = Number(/--gantt-axis-h:\s*(\d+(?:\.\d+)?)px/.exec(tokens)![1]);

/** A `#rrggbb` theme token as [r, g, b]. */
export function rgb(hex: string): readonly [number, number, number] {
  const n = Number.parseInt(hex.slice(1, 7), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

/** CSS px, row height: read from the `--gantt-row-h` token in tokens.css, the default `Viewport::row_height` mirrors. */
export const ROW = Number(/--gantt-row-h:\s*(\d+(?:\.\d+)?)px/.exec(tokens)![1]);

/** A decoded screenshot: `px` holds `channels` bytes per pixel, row by row. */
export interface Png {
  width: number;
  height: number;
  channels: number;
  px: Buffer;
}

/** Minimal PNG decoder: 8-bit RGB or RGBA, non-interlaced (what Chrome writes). */
export function decodePng(buf: Buffer): Png {
  let pos = 8;
  let width = 0;
  let height = 0;
  let channels = 0;
  const idat: Buffer[] = [];
  while (pos < buf.length) {
    const len = buf.readUInt32BE(pos);
    const type = buf.toString("ascii", pos + 4, pos + 8);
    const data = buf.subarray(pos + 8, pos + 8 + len);
    if (type === "IHDR") {
      width = data.readUInt32BE(0);
      height = data.readUInt32BE(4);
      if (data[8] !== 8 || data[12] !== 0) throw new Error("unsupported PNG");
      const colourChannels: Record<number, number> = { 2: 3, 6: 4 };
      channels = colourChannels[data[9]] ?? 0;
      if (!channels) throw new Error(`unsupported PNG colour type ${data[9]}`);
    } else if (type === "IDAT") {
      idat.push(data);
    }
    pos += 12 + len;
  }
  const raw = inflateSync(Buffer.concat(idat));
  const stride = width * channels;
  const px = Buffer.alloc(height * stride);
  for (let y = 0; y < height; y++) {
    const f = raw[y * (stride + 1)];
    const src = raw.subarray(y * (stride + 1) + 1, (y + 1) * (stride + 1));
    for (let i = 0; i < stride; i++) {
      const a = i >= channels ? px[y * stride + i - channels] : 0;
      const b = y > 0 ? px[(y - 1) * stride + i] : 0;
      const c = y > 0 && i >= channels ? px[(y - 1) * stride + i - channels] : 0;
      let v = src[i];
      if (f === 1) v += a;
      else if (f === 2) v += b;
      else if (f === 3) v += (a + b) >> 1;
      else if (f === 4) {
        const p = a + b - c;
        const pa = Math.abs(p - a);
        const pb = Math.abs(p - b);
        const pc = Math.abs(p - c);
        v += pa <= pb && pa <= pc ? a : pb <= pc ? b : c;
      }
      px[y * stride + i] = v & 0xff;
    }
  }
  return { width, height, channels, px };
}

/** Pixels within `tol` per channel of one colour, below the axis band or inside `rect` (CSS px [x0, y0, x1, y1]). */
export function countColour(
  png: Png,
  [R, G, B]: readonly [number, number, number],
  tol: number,
  [x0, y0, x1, y1]: readonly [number, number, number, number] = [0, AXIS, png.width / dpr, png.height / dpr],
): number {
  let count = 0;
  for (let y = Math.max(0, Math.round(y0 * dpr)); y < Math.min(png.height, Math.round(y1 * dpr)); y++) {
    for (let x = Math.max(0, Math.round(x0 * dpr)); x < Math.min(png.width, Math.round(x1 * dpr)); x++) {
      const i = (y * png.width + x) * png.channels;
      if (Math.abs(png.px[i] - R) <= tol && Math.abs(png.px[i + 1] - G) <= tol && Math.abs(png.px[i + 2] - B) <= tol) count++;
    }
  }
  return count;
}
