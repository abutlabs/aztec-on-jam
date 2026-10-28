// Serves the built wallet (dist/) cross-origin isolated: bb.js proves on threads, which need
// SharedArrayBuffer, which needs these headers. The page at / is the wallet, or the explorer.
//   node serve.mjs [port] [page]   (default 19720 index.html; the explorer: 19730 explorer.html)
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { extname, join, normalize } from 'node:path';
import { fileURLToPath } from 'node:url';

const DIST = join(fileURLToPath(new URL('.', import.meta.url)), 'dist');
const PORT = Number(process.argv[2] ?? 19720);
const PAGE = process.argv[3] ?? 'index.html';
const TYPES = { '.html': 'text/html', '.js': 'text/javascript', '.mjs': 'text/javascript',
  '.css': 'text/css', '.wasm': 'application/wasm', '.json': 'application/json', '.map': 'application/json' };

createServer(async (req, res) => {
  const path = normalize(new URL(req.url, 'http://x').pathname).replace(/^(\.\.[/\\])+/, '');
  const file = join(DIST, path === '/' ? PAGE : path);
  try {
    const body = await readFile(file);
    res.writeHead(200, {
      'content-type': TYPES[extname(file)] ?? 'application/octet-stream',
      'cross-origin-opener-policy': 'same-origin',
      'cross-origin-embedder-policy': 'require-corp',
      'cross-origin-resource-policy': 'cross-origin',
      'cache-control': 'no-cache',
    });
    res.end(body);
  } catch {
    res.writeHead(404).end('not found');
  }
}).listen(PORT, () => console.log(`${PAGE === 'index.html' ? 'wallet' : PAGE} on http://localhost:${PORT}`));
