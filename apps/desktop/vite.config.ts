import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  build: {
    rollupOptions: {
      output: {
        // The workbench always owns exactly one renderer, but its Three.js
        // runtime and icon set do not need to share the application entry
        // chunk. Keep those stable vendor boundaries so a focused V003 card
        // cannot push the boot chunk over the T003 budget.
        manualChunks(id) {
          if (id.includes('/node_modules/three/')) return 'three-runtime'
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
