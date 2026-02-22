import { defineConfig } from '@rsbuild/core';

export default defineConfig({
  source: {
    entry: {
      index: './src/main.ts',
    },
  },
  html: {
    template: './index.html',
  },
  tools: {
    postcss: {
      postcssOptions: {
        plugins: [
          require('tailwindcss'),
          require('autoprefixer'),
        ],
      },
    },
    rspack: {},
  },
  server: {
    port: 3555,
  },
  output: {
    distPath: {
      root: '../target/live-editor',
    },
    copy: [
      {
        from: './public',
        to: '.',
      },
    ],
  },
});
