import { resolve } from 'path'
import tailwindcss from '@tailwindcss/vite'
import react from '@vitejs/plugin-react'
import { defineConfig, normalizePath, type Plugin } from 'vite'

const renderer = resolve('src/renderer')
const bridge = normalizePath(resolve('src/tauri/bridge.ts'))
// Vite module ids use forward slashes, also on Windows.
const entry = normalizePath(resolve(renderer, 'src/main.tsx'))

/** Loads the bridge ahead of the renderer, which only knows `window.echoframe`. */
function tauriBridge(): Plugin {
  return {
    name: 'rikette-bridge',
    transform(code, id) {
      if (id !== entry) return null
      return { code: `import ${JSON.stringify(bridge)}\n${code}`, map: null }
    }
  }
}

// The app's pages, served to Tauri (src-tauri/tauri.conf.json): the main window, the save
// HUD and the launch splash.
export default defineConfig({
  root: renderer,
  base: './',
  clearScreen: false,
  resolve: {
    alias: {
      '@': resolve('src/renderer/src'),
      '@renderer': resolve('src/renderer/src'),
      '@echoframe/shared': resolve('../../packages/shared/src/index.ts')
    }
  },
  plugins: [tauriBridge(), react(), tailwindcss()],
  server: {
    port: 5174,
    strictPort: true
  },
  build: {
    outDir: resolve('out/tauri'),
    emptyOutDir: true,
    target: 'es2022',
    rollupOptions: {
      input: {
        index: resolve(renderer, 'index.html'),
        overlay: resolve(renderer, 'overlay.html'),
        splash: resolve(renderer, 'splash.html')
      }
    }
  }
})
