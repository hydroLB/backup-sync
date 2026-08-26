import { defineConfig, Plugin } from 'vite';
import react from '@vitejs/plugin-react';
import { sites } from '@openai/sites-vite-plugin';
import { cloudflare } from '@cloudflare/vite-plugin';
import { fileURLToPath } from 'node:url';

const WEB_TITLE = 'Backup Sync — Content-addressed local backups';
const WEB_DESCRIPTION =
  'A fully interactive browser edition of Backup Sync with real hashing, deduplication, immutable versions, integrity verification, replication repair, and restore.';
const WEB_CSP =
  "default-src 'self'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'self' data: blob:; font-src 'self' data:; object-src 'none'; base-uri 'none'; form-action 'none'; worker-src 'self' blob:";

function webMetadata(includeSecurityMeta: boolean): Plugin {
  return {
    name: 'backup-sync-web-metadata',
    transformIndexHtml: {
      order: 'pre',
      handler(html) {
        const siteUrl = process.env.VITE_PUBLIC_SITE_URL?.replace(/\/$/, '');
        const tags = [
          ...(includeSecurityMeta
            ? [
                {
                  tag: 'meta',
                  attrs: { 'http-equiv': 'Content-Security-Policy', content: WEB_CSP },
                  injectTo: 'head' as const,
                },
              ]
            : []),
          {
            tag: 'meta',
            attrs: { name: 'description', content: WEB_DESCRIPTION },
            injectTo: 'head' as const,
          },
          {
            tag: 'meta',
            attrs: { name: 'theme-color', content: '#050a16' },
            injectTo: 'head' as const,
          },
          {
            tag: 'meta',
            attrs: { property: 'og:type', content: 'website' },
            injectTo: 'head' as const,
          },
          {
            tag: 'meta',
            attrs: { property: 'og:title', content: WEB_TITLE },
            injectTo: 'head' as const,
          },
          {
            tag: 'meta',
            attrs: { property: 'og:description', content: WEB_DESCRIPTION },
            injectTo: 'head' as const,
          },
          {
            tag: 'meta',
            attrs: { name: 'twitter:card', content: 'summary_large_image' },
            injectTo: 'head' as const,
          },
          {
            tag: 'meta',
            attrs: { name: 'twitter:title', content: WEB_TITLE },
            injectTo: 'head' as const,
          },
          {
            tag: 'meta',
            attrs: { name: 'twitter:description', content: WEB_DESCRIPTION },
            injectTo: 'head' as const,
          },
          {
            tag: 'link',
            attrs: { rel: 'icon', type: 'image/png', href: '/favicon.png' },
            injectTo: 'head' as const,
          },
        ];
        if (siteUrl) {
          tags.push(
            {
              tag: 'meta',
              attrs: { property: 'og:url', content: siteUrl },
              injectTo: 'head' as const,
            },
            {
              tag: 'meta',
              attrs: { property: 'og:image', content: `${siteUrl}/og.png` },
              injectTo: 'head' as const,
            },
            {
              tag: 'meta',
              attrs: { name: 'twitter:image', content: `${siteUrl}/og.png` },
              injectTo: 'head' as const,
            },
          );
        }
        return {
          html: html.replace('<title>Backup Sync</title>', `<title>${WEB_TITLE}</title>`),
          tags,
        };
      },
    },
  };
}

export default defineConfig(({ command }) => {
  const webRuntime = process.env.VITE_APP_RUNTIME === 'web';

  return {
    plugins: [
      react(),
      ...(webRuntime ? [webMetadata(command === 'build'), sites(), cloudflare()] : []),
    ],
    resolve: webRuntime
      ? {
          alias: {
            '@tauri-apps/api/core': fileURLToPath(
              new URL('./src/runtime/web/tauriCoreStub.ts', import.meta.url),
            ),
            '@tauri-apps/plugin-dialog': fileURLToPath(
              new URL('./src/runtime/web/tauriDialogStub.ts', import.meta.url),
            ),
          },
        }
      : undefined,
    server: {
      host: '127.0.0.1',
      port: 5173,
      strictPort: true,
    },
  };
});
