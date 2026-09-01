import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { defineConfig } from 'vite'

const demoDir = dirname(fileURLToPath(import.meta.url))

// Keep the package root as the Vite serving root so the demo can verify and
// consume the immutable manifest/GLB without copying either asset.
export default defineConfig({
  root: resolve(demoDir, '..'),
  server: {
    fs: { strict: true },
  },
  build: {
    outDir: resolve(demoDir, 'dist'),
    emptyOutDir: true,
    rollupOptions: {
      input: resolve(demoDir, 'index.html'),
    },
  },
})
