import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  build: {
    // Keep runtime and renderer chunks bounded so the first-view payload
    // stays predictable on desktops where the workbench may stay idle.
    chunkSizeWarningLimit: 560,
    rollupOptions: {
      output: {
        manualChunks(id) {
          if (id.includes('/node_modules/three/examples/jsm/utils/')) {
            const utilRoot = '/node_modules/three/examples/jsm/utils/'
            const utilIndex = id.indexOf(utilRoot)
            const file = id.slice(utilIndex + utilRoot.length).split('/')[0].replace('.js', '').replaceAll('-', '_')
            return `three-extras-utils-${file}`
          }
          if (id.includes('/node_modules/three/examples/jsm/controls/')) return 'three-extras-controls'
          if (id.includes('/node_modules/three/examples/jsm/loaders/')) {
            const loaderRoot = '/node_modules/three/examples/jsm/loaders/'
            const loaderIndex = id.indexOf(loaderRoot)
            const file = id.slice(loaderIndex + loaderRoot.length).split('/')[0].replace('.js', '').replaceAll('-', '_')
            return `three-extras-loaders-${file}`
          }
          if (id.includes('/node_modules/three/examples/jsm/helpers/')) return 'three-extras-helpers'
          if (id.includes('/node_modules/three/examples/jsm/')) {
            const threeExamplesPath = '/node_modules/three/examples/jsm/'
            const examplesIndex = id.indexOf(threeExamplesPath)
            const segment = id.slice(examplesIndex + threeExamplesPath.length).split('/')[0]
            const safeSegment = segment.replaceAll('-', '_')
            return `three-extras-${safeSegment}`
          }
          if (id.includes('/node_modules/@phosphor-icons/react/')) return 'phosphor-icons'
          // OrbitControls/GLTFLoader import the public three entry, which also
          // exports WebGLRenderer. Keep that bridge with the renderer chunk so
          // the lightweight source-only core cannot point back to it.
          if (id.includes('/node_modules/three/build/three.module.js')) return 'three-runtime-renderer'
          const threeSourceRoot = '/node_modules/three/src/'
          const sourceIndex = id.indexOf(threeSourceRoot)
          if (sourceIndex >= 0) {
            if (id.endsWith('/three/src/extras/PMREMGenerator.js')) return 'three-runtime-renderer'
            if (id.endsWith('/three/src/materials/ShaderMaterial.js')) return 'three-runtime-renderer'
            if (id.endsWith('/three/src/materials/RawShaderMaterial.js')) return 'three-runtime-renderer'
            if (id.includes('/node_modules/three/src/renderers/shaders/')) return 'three-runtime-shaders'
            if (id.includes('/node_modules/three/src/renderers/')) return 'three-runtime-renderer'
            return 'three-runtime-core'
          }
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
