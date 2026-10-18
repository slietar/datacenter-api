import path from 'node:path';
import react from '@vitejs/plugin-react';
import { defineConfig } from 'vite';

// https://vite.dev/config/
export default defineConfig({
  server: {
    fs: {
      // Linked ../../ui packages pull assets (e.g. @fontsource) from their workspace's node_modules.
      allow: ['.', path.resolve(__dirname, '../../ui')],
    },
  },
  resolve: {
    // The linked ../../ui packages have their own node_modules/react; force a single copy.
    dedupe: ['react', 'react-dom'],
    alias: {
      react: path.resolve(__dirname, 'node_modules/react'),
      'react-dom': path.resolve(__dirname, 'node_modules/react-dom'),
    },
  },
  plugins: [
    react({
      babel: {
        plugins: [['babel-plugin-react-compiler']],
      },
    }),
  ],
})
