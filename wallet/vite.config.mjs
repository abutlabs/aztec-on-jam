// The wallet's build. Aztec's browser stack needs three things from the bundler: Buffer and
// path polyfills, the sqlite OPFS runtime files served under their own names (its loader asks
// for them by runtime-built paths no bundler can rewrite), and cross-origin isolation when a
// dev server runs (bb.js proves on threads, which need SharedArrayBuffer). serve.mjs sets the
// same headers for the built site.
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { nodePolyfills } from 'vite-plugin-node-polyfills';

const require = createRequire(import.meta.url);

// vite-plugin-node-polyfills#81: its shim ids stay unresolved inside dependency chunks
const polyfills = () => ({
  ...nodePolyfills({ include: ['buffer', 'path'] }),
  resolveId(id) {
    if (/^vite-plugin-node-polyfills\/shims\/(buffer|global|process)$/.test(id)) return require.resolve(id);
  },
});

const sqliteRuntimeFiles = () => ({
  name: 'sqlite-runtime-files',
  apply: 'build',
  generateBundle() {
    for (const f of ['sqlite3.wasm', 'sqlite3-opfs-async-proxy.js']) {
      this.emitFile({
        type: 'asset',
        fileName: `assets/${f}`,
        source: readFileSync(require.resolve(`@aztec-labs/sqlite3mc-wasm/vendor/jswasm/${f}`)),
      });
    }
  },
});

const isolation = {
  'Cross-Origin-Opener-Policy': 'same-origin',
  'Cross-Origin-Embedder-Policy': 'require-corp',
  'Cross-Origin-Resource-Policy': 'cross-origin',
};

export default {
  plugins: [polyfills(), sqliteRuntimeFiles()],
  define: { 'process.env': '{}' },
  // two pages: the wallet, and the explorer (served on its own port, so its own origin and storage)
  build: { target: 'esnext', chunkSizeWarningLimit: 12000, rollupOptions: { input: { wallet: 'index.html', explorer: 'explorer.html' } } },
  server: { headers: isolation },
  preview: { headers: isolation },
};
