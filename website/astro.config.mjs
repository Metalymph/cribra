// @ts-check

import { unified } from '@astrojs/markdown-remark';
import mdx from '@astrojs/mdx';
import sitemap from '@astrojs/sitemap';
import expressiveCode from 'astro-expressive-code';
import { defineConfig } from 'astro/config';
import process from 'node:process';
import { loadEnv } from 'vite';
import rehypeAutolinkHeadings from 'rehype-autolink-headings';
import rehypeSlug from 'rehype-slug';
import { toneExpressiveCodeOptions } from './src/config/expressive-code.ts';

// https://astro.build/config
const env = loadEnv(process.env.NODE_ENV || 'production', process.cwd(), 'ASTRO_');
const configuredSite = process.env.ASTRO_SITE_URL || env.ASTRO_SITE_URL || undefined;
if (configuredSite && !/^https?:\/\/[^/]+\/?$/.test(configuredSite)) {
  throw new Error('ASTRO_SITE_URL must be an HTTP(S) origin without a path.');
}

export default defineConfig({
  site: configuredSite,
  output: 'static',
  trailingSlash: 'always',
  integrations: [
    expressiveCode(toneExpressiveCodeOptions),
    mdx(),
    ...(configuredSite
      ? [
          sitemap({
            filter: (page) => !['/search/', '/404/', '/404.html'].includes(new URL(page).pathname),
          }),
        ]
      : []),
  ],
  build: {
    inlineStylesheets: 'always',
  },

  // Astro 7 defaults `compressHTML` to 'jsx'. The theme is authored against the
  // previous HTML-aware whitespace handling, so keep that behavior explicitly.
  compressHTML: true,

  markdown: {
    // Astro 7 defaults to the Sätteri Markdown processor. The theme relies on
    // rehype plugins, so opt back into the unified pipeline.
    processor: unified({
      rehypePlugins: [
        rehypeSlug,
        [
          rehypeAutolinkHeadings,
          {
            behavior: 'append',
            properties: { ariaHidden: true, tabIndex: -1, class: 'heading-anchor' },
            content: { type: 'text', value: '#' },
          },
        ],
      ],
    }),
  },
});
