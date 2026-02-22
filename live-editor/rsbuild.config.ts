import { defineConfig } from '@rsbuild/core';

const publicPath = process.env.PUBLIC_PATH || '/';

export default defineConfig({
  source: {
    entry: {
      index: './src/main.ts',
    },
  },
  html: {
    template: './index.html',
    tags: publicPath !== '/'
      ? [{ tag: 'base', attrs: { href: publicPath }, head: true, append: false }]
      : [],
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
    assetPrefix: publicPath,
    copy: [
      {
        from: './public',
        to: '.',
      },
    ],
  },
});
