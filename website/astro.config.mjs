// @ts-check
import { defineConfig } from 'astro/config';
import sitemap from '@astrojs/sitemap';

// https://astro.build/config
export default defineConfig({
  site: 'https://token-editor.com',
  integrations: [sitemap()],
  prefetch: {
    prefetchAll: true,
  },
});
