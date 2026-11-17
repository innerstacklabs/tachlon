import { createServer } from "node:http";
import { readFile, realpath } from "node:fs/promises";
import { extname, resolve, sep } from "node:path";

const types = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".wasm": "application/wasm",
  ".css": "text/css; charset=utf-8",
  ".xml": "application/xml; charset=utf-8",
  ".json": "application/json; charset=utf-8",
};

export async function serveDir(dir) {
  const root = await realpath(resolve(dir));
  const server = createServer(async (req, res) => {
    let pathname;
    try {
      pathname = decodeURIComponent((req.url ?? "/").split("?", 1)[0]);
    } catch {
      res.writeHead(400).end("bad request");
      return;
    }
    const relative = pathname === "/" ? "index.html" : `.${pathname}`;
    const file = resolve(root, relative);
    if (file !== root && !file.startsWith(`${root}${sep}`)) {
      res.writeHead(403).end("forbidden");
      return;
    }
    try {
      const actual = await realpath(file);
      if (actual !== root && !actual.startsWith(`${root}${sep}`)) {
        res.writeHead(403).end("forbidden");
        return;
      }
      const body = await readFile(actual);
      res.writeHead(200, { "content-type": types[extname(file).toLowerCase()] ?? "application/octet-stream" }).end(body);
    } catch {
      res.writeHead(404).end("not found");
    }
  });
  await new Promise((resolveListen, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolveListen);
  });
  const { port } = server.address();
  return {
    port,
    close: () => new Promise((resolveClose, reject) => server.close((err) => (err ? reject(err) : resolveClose()))),
  };
}
