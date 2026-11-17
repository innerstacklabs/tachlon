import { createServer } from "node:http";
import type { Server } from "node:http";
import { readFile } from "node:fs/promises";
import { extname, join, normalize, sep } from "node:path";
import { expect, test } from "@playwright/test";

// The book's Next and Previous links (gs#703): mdBook hides its side arrows when the sidebar is open at 1380 px or less, so
// at the demo's 1280 px window the Welcome page showed no way forward above the fold. book/theme/book.css restores them
// from 1280 px. This spec serves a built book (`mdbook build book -d <dir>`, the folder in BOOK_DIR) and reads the links.

const BOOK_DIR = process.env.BOOK_DIR;
if (!BOOK_DIR) throw new Error("BOOK_DIR is not set: build the book with `mdbook build book -d <dir>` and point BOOK_DIR at it");
const ROOT = normalize(BOOK_DIR);

const TYPES: Record<string, string> = {
  ".html": "text/html; charset=utf-8",
  ".css": "text/css",
  ".js": "text/javascript",
  ".svg": "image/svg+xml",
  ".png": "image/png",
  ".json": "application/json",
  ".woff2": "font/woff2",
};

let server: Server;
let base = "";

test.beforeAll(async () => {
  server = createServer((request, response) => {
    const path = decodeURIComponent((request.url ?? "/").split("?")[0]!);
    const file = normalize(join(ROOT, path.endsWith("/") ? `${path}index.html` : path));
    if (file !== ROOT && !file.startsWith(ROOT + sep)) {
      response.writeHead(403).end();
      return;
    }
    readFile(file).then(
      (body) => response.writeHead(200, { "content-type": TYPES[extname(file)] ?? "application/octet-stream" }).end(body),
      () => response.writeHead(404).end(),
    );
  });
  await new Promise<void>((done) => server.listen(0, "127.0.0.1", done));
  const address = server.address();
  if (!address || typeof address === "string") throw new Error("no server address");
  base = `http://127.0.0.1:${address.port}/`;
});

test.afterAll(async () => {
  await new Promise<void>((done) => server.close(() => done()));
});

const CASES = [
  { page: "index.html", width: 1280 },
  { page: "index.html", width: 1380 },
  { page: "open-a-schedule.html", width: 1280 },
];

for (const { page: name, width } of CASES) {
  test(`${name} at ${width} px shows its Next link on the first screen`, async ({ page }) => {
    await page.setViewportSize({ width, height: 800 });
    await page.goto(base + name);
    await expect(page.locator('a[rel~="next"]:visible')).toBeInViewport({ ratio: 1 });
    if (name !== "index.html") await expect(page.locator('a[rel~="prev"]:visible')).toBeInViewport({ ratio: 1 });
  });
}
