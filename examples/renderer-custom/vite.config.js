// Copyright 2025 StrongDM Inc
// SPDX-License-Identifier: Apache-2.0

import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  build: {
    lib: {
      entry: 'renderer.jsx',
      formats: ['es'],
      fileName: 'renderer',
    },
    rollupOptions: {
      // Externalize React - the CXDB UI provides it
      external: ['react', 'react/jsx-runtime'],
      output: {
        globals: {
          react: 'React',
        },
      },
    },
    // Don't minify for easier debugging
    minify: false,
  },
});
