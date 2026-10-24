export function GET({ site }) {
  return new Response(
    site
      ? `User-agent: *\nAllow: /\n\nSitemap: ${new URL('/sitemap-index.xml', site).href}\n`
      : 'User-agent: *\nDisallow: /\n',
    {
      headers: { 'Content-Type': 'text/plain; charset=utf-8' },
    }
  );
}
