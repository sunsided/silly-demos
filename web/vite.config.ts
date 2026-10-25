import process from 'node:process'
import adapter from '@sveltejs/adapter-static'
import { sveltekit } from '@sveltejs/kit/vite'
import { defineConfig } from 'vite'

// GitHub Pages serves the site from /<repo>; CI sets BASE_PATH to that.
const base = (process.env.BASE_PATH ?? '') as '' | `/${string}`

export default defineConfig({
  plugins: [
    sveltekit({
      // Every page is prerendered; 404.html replaces GitHub Pages' own error page.
      adapter: adapter({ fallback: '404.html' }),
      paths: { base: process.argv.includes('dev') ? '' : base },
    }),
  ],
  server: {
    port: 3000,
    host: true,
    // The WASM package is built into ../pkg by wasm-pack.
    fs: { allow: ['..'] },
  },
  resolve: {
    alias: {
      silly_demos: new URL('../pkg/silly_demos.js', import.meta.url).pathname,
    },
  },
})
