import { createServer, type Server } from "node:http";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const FIXTURE = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "fixture-app",
);

const TYPES: Record<string, string> = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".css": "text/css; charset=utf-8",
};

export async function startFixtureServer(): Promise<{
  url: string;
  close: () => Promise<void>;
}> {
  const server: Server = createServer(async (req, res) => {
    try {
      const urlPath = (req.url ?? "/").split("?")[0] || "/";
      const rel = urlPath === "/" ? "/index.html" : urlPath;
      const file = path.normalize(path.join(FIXTURE, rel));
      if (!file.startsWith(FIXTURE)) {
        res.writeHead(403).end("forbidden");
        return;
      }
      const body = await readFile(file);
      const ext = path.extname(file);
      res.writeHead(200, { "content-type": TYPES[ext] ?? "application/octet-stream" });
      res.end(body);
    } catch {
      res.writeHead(404).end("not found");
    }
  });

  await new Promise<void>((resolve) => server.listen(0, "127.0.0.1", resolve));
  const addr = server.address();
  if (!addr || typeof addr === "string") throw new Error("no address");
  return {
    url: `http://127.0.0.1:${addr.port}/`,
    close: () =>
      new Promise((resolve, reject) => {
        server.close((err) => (err ? reject(err) : resolve()));
      }),
  };
}

export const testRubricPath = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "rubric.v1.json",
);
