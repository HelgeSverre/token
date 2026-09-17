// @ts-check
import { defineConfig } from 'astro/config';
import sitemap from '@astrojs/sitemap';
import {createReadStream} from 'node:fs';
import {stat} from 'node:fs/promises';
import {join} from 'node:path';
import {privateArchiveRoot} from './src/lib/private-archive.mjs';

const archiveRoot = privateArchiveRoot();

// https://astro.build/config
export default defineConfig({
  site: 'https://token-editor.com',
  integrations: [...(archiveRoot ? [{
    name: 'private-agent-archive',
    hooks: {
      'astro:config:setup': ({command, updateConfig}) => {
        if (command !== 'dev') throw new Error('Private transcripts can only run in dev. Unset AGENT_ARCHIVE_PREVIEW for a public build.');
        updateConfig({server: {host: '127.0.0.1'}, vite: {plugins: [{
          name: 'private-agent-assets',
          configureServer(server) {
            // Reload when export writes its completed receipt, rather than on each session.
            const receipt = join(archiveRoot, 'index.json');
            server.watcher.add(receipt);
            const refresh = file => {
              if (file !== receipt) return;
              server.moduleGraph.invalidateAll();
              server.ws.send({type: 'full-reload'});
            };
            server.watcher.on('change', refresh);
            server.httpServer?.once('close', () => server.watcher.off('change', refresh));
            server.middlewares.use(async (request, response, next) => {
              const pathname = (request.url || '').split('?')[0];
              if (!pathname.startsWith('/agent-assets/')) return next();
              if (!/^\/agent-assets\/[a-f0-9]{64}\.txt$/.test(pathname)) { response.statusCode = 404; response.end(); return; }
              const file = join(archiveRoot, pathname.slice(1));
              try {
                const info = await stat(file);
                response.setHeader('Content-Type', 'text/plain; charset=utf-8');
                response.setHeader('Content-Length', info.size);
                response.setHeader('Cache-Control', 'no-store');
                response.setHeader('X-Content-Type-Options', 'nosniff');
                const stream = createReadStream(file);
                stream.on('error', () => response.destroy());
                stream.pipe(response);
              } catch { response.statusCode = 404; response.end(); }
            });
          },
        }]}});
      },
    },
  }] : []), sitemap({
    // Transcript chunks and the catalogue JSON are implementation assets. They
    // are deliberately fetchable but must never become discoverable documents.
    filter: page => {
      const path = new URL(page).pathname;
      return !path.startsWith('/agent-chunks/') && path !== '/agent-index.json';
    },
  })],
  prefetch: {
    prefetchAll: true,
  },
});
