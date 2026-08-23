import { createReadStream } from "node:fs";
import { stat } from "node:fs/promises";
import { createServer } from "node:http";
import { extname, resolve, sep } from "node:path";

const root = process.cwd();
const port = Number(process.env.CRIBRA_WASM_BENCH_PORT ?? 8787);

const contentTypes = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".mjs": "text/javascript; charset=utf-8",
  ".wasm": "application/wasm",
  ".json": "application/json; charset=utf-8",
};

const server = createServer(async (request, response) => {
  const url = new URL(request.url ?? "/", "http://127.0.0.1");
  const requested = resolve(root, `.${decodeURIComponent(url.pathname)}`);

  if (requested !== root && !requested.startsWith(`${root}${sep}`)) {
    response.writeHead(400);
    response.end("bad path");
    return;
  }

  try {
    const info = await stat(requested);

    if (!info.isFile()) {
      throw new Error("not a file");
    }

    response.writeHead(200, {
      "Content-Type":
        contentTypes[extname(requested)] ?? "application/octet-stream",
      "Cache-Control": "no-store",
      "Cross-Origin-Opener-Policy": "same-origin",
      "Cross-Origin-Embedder-Policy": "require-corp",
    });

    createReadStream(requested).pipe(response);
  } catch {
    response.writeHead(404);
    response.end("not found");
  }
});

server.listen(port, "127.0.0.1", () => {
  console.log(
    `Cribra WASM benchmark: http://127.0.0.1:${port}/crates/cribra-wasm/benches/web/bench.html`,
  );
});