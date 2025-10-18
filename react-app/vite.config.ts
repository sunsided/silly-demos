import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import { fileURLToPath } from "url";
import path from 'path';

export default defineConfig(({ mode }) => {
  // Get base path from environment variable or use default based on mode
  const envBasePath = process.env.VITE_BASE_PATH;
  let basePath: string;
  
  if (envBasePath) {
    basePath = envBasePath.endsWith('/') ? envBasePath : `${envBasePath}/`;
  } else {
    basePath = mode === 'production' ? '/silly-demos/' : '/';
  }

  return {
    plugins: [react()],
    base: basePath,
    build: {
      outDir: 'dist',
      assetsDir: 'assets',
      rollupOptions: {
        output: {
          assetFileNames: (assetInfo) => {
            // Keep WASM files in assets root for easier loading
            if (assetInfo.name?.endsWith('.wasm')) {
              return 'assets/[name][extname]'
            }
            return 'assets/[name]-[hash][extname]'
          }
        }
      }
    },
    server: {
      port: 3000,
      host: true,
      fs: {
        allow: [
          '..', // allow one level up for WASM imports
          'src',
        ],
      },
    },
    assetsInclude: ['**/*.wasm'],
    optimizeDeps: {
      exclude: ['silly-demos'],
      include: [
        '../../pkg/silly_demos.js',
      ],
    },
    resolve: {
      alias: {
        // Correct path for glue code relative to vite.config.ts in react-app/
        "silly_demos": path.resolve(__dirname, '../pkg/silly_demos.js')
      },
    },
  };
});
