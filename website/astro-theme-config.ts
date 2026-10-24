/** Site identity and navigation; production origin is supplied by ASTRO_SITE_URL. */
const config = {
  site: {
    lang: 'en',
    locale: 'en_US',
    dateLocale: 'en-US',
    title: 'Cribra',
    logoLabel: 'Cribra',
    description:
      'An embeddable, local-first Rust engine for deterministic sensitive-data detection, validation, and safe transformation.',
    author: 'Cribra contributors',
  },
  nav: [
    { label: 'Docs', href: '/docs/' },
    { label: 'Bindings', href: '/bindings/' },
    { label: 'Examples', href: '/examples/' },
  ],
  footerNav: [
    { label: 'Docs', href: '/docs/' },
    { label: 'GitHub', href: 'https://github.com/Metalymph/cribra' },
    { label: 'Search', href: '/search/' },
  ],
  behavior: { smoothScroll: true },
};

export default config;
