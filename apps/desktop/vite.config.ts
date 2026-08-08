import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  build: {
    rollupOptions: {
      output: {
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
