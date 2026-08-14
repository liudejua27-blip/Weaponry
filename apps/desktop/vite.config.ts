import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  build: {
    // The remaining Three.js renderer chunk is lazy-loaded only after a
    // candidate with a GLB is selected; keep the warning threshold aligned
    // with that intentional desktop-only vendor chunk.
    chunkSizeWarningLimit: 600,
    rollupOptions: {
      output: {
        manualChunks(id) {
          if (id.includes('/node_modules/three/examples/jsm/')) return 'three-extras'
          if (id.includes('/node_modules/three/')) return 'three-core'
          if (id.includes('/node_modules/@phosphor-icons/react/')) return 'phosphor-icons'
          return undefined
        },
      },
    },
  },
  server: {
    port: 1420,
    strictPort: true,
  },
})
