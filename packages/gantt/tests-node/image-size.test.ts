// The size Save image draws a printed page at: twice the page's own viewBox (gantt#1455).
import assert from "node:assert/strict";
import { test } from "node:test";
import { imageSize } from "../src/image-export.ts";

test("the image is twice the page's viewBox: 2112 by 1632 for Letter, 2246 by 1588 for A4", () => {
  const page = (box: string): string => `<svg xmlns="http://www.w3.org/2000/svg" viewBox="${box}"><rect/></svg>`;
  assert.deepEqual(imageSize(page("0 0 1056 816")), { width: 2112, height: 1632 });
  assert.deepEqual(imageSize(page("0 0 1123 794")), { width: 2246, height: 1588 });
  assert.throws(() => imageSize("<svg><rect/></svg>"), /viewBox/);
});
