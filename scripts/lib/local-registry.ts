// A local package server (gantt#1560, build row B5): serves the packument and the tarball of the unreleased package, with its integrity,
// and redirects every other package to npmjs, so `npm install --registry <url> <name>` is a real install of a build that is not published.
//   node scripts/lib/local-registry.ts <tarball> <version> [port]     (the tarball is the .tgz that `npm pack` wrote)
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { createServer, type Server } from "node:http";
import { PRODUCT_NAME } from "../../packages/gantt/src/product.ts";

export const NAME = PRODUCT_NAME.toLowerCase();
export const PORT = 4871; // never 4173 or 4300 (lifeos#3278)

/** Starts the server on 127.0.0.1 and resolves once it listens. */
export function startRegistry(tarball: string, version: string, port = PORT): Promise<Server> {
  const bytes = readFileSync(tarball);
  const file = `${NAME}-${version}.tgz`;
  // npm reads these from the listing, not the tarball, at install: without `bin` it links no node_modules/.bin/<name> (gantt#1688).
  const manifest = JSON.parse(execFileSync("tar", ["-xzOf", tarball, "package/package.json"], { encoding: "utf8" })) as Record<string, unknown>;
  const installFields = Object.fromEntries(
    ["bin", "dependencies", "peerDependencies", "engines"].filter((key) => key in manifest).map((key) => [key, manifest[key]]),
  );
  const packument = JSON.stringify({
    name: NAME,
    "dist-tags": { latest: version },
    versions: {
      [version]: {
        ...installFields,
        name: NAME,
        version,
        dist: {
          tarball: `http://127.0.0.1:${port}/${NAME}/-/${file}`,
          integrity: `sha512-${createHash("sha512").update(bytes).digest("base64")}`,
          shasum: createHash("sha1").update(bytes).digest("hex"),
        },
      },
    },
  });
  const server = createServer((req, res) => {
    const path = (req.url ?? "/").split("?")[0];
    if (path === `/${NAME}`) res.writeHead(200, { "content-type": "application/json" }).end(packument);
    else if (path === `/${NAME}/-/${file}`) res.writeHead(200, { "content-type": "application/octet-stream" }).end(bytes);
    else res.writeHead(302, { location: `https://registry.npmjs.org${req.url}` }).end();
  });
  return new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(port, "127.0.0.1", () => resolve(server));
  });
}

if (import.meta.main) {
  const [tarball, version, port] = process.argv.slice(2);
  if (!tarball || !version) {
    console.error("usage: node scripts/lib/local-registry.ts <tarball> <version> [port]");
    process.exit(2);
  }
  const at = port ? Number(port) : PORT;
  await startRegistry(tarball, version, at);
  console.log(`local registry: npm install --registry http://127.0.0.1:${at} ${NAME}`);
}
