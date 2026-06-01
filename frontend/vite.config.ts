import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import path from 'path'

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  server: {
    host: '0.0.0.0',
    port: 5173,
    proxy: {
      '/api': 'http://localhost:7777',
      '/ws': {
        target: 'ws://localhost:7777',
        ws: true,
      },
    },
  },
  build: {
    rollupOptions: {
      output: {
        // Split the heavy CosmJS/protobuf stack out of the main app chunk so
        // the initial load isn't a single ~3 MB bundle. These deps change far
        // less often than app code, so they also cache better across deploys.
        manualChunks(id) {
          if (id.includes('node_modules')) {
            if (id.includes('@cosmjs') || id.includes('cosmjs-types')) return 'cosmjs'
            if (id.includes('protobufjs') || id.includes('@protobufjs')) return 'protobuf'
            if (id.includes('react') || id.includes('scheduler')) return 'react-vendor'
          }
        },
      },
    },
  },
})
