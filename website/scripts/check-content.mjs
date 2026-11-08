#!/usr/bin/env node

/**
 * Dependency-free content and publication-boundary validation for the Cribra
 * website. Runs after `astro build && pagefind --site dist`.
 *
 * The Pagefind check inspects the generated fragment metadata and asserts that
 * each documentation URL is present. It never compares a global Pagefind page
 * count with the number of Markdown documents: the site indexes other pages too.
 */

import { readFile, readdir, stat } from 'node:fs/promises';
import { gunzipSync } from 'node:zlib';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const scriptDirectory = path.dirname(fileURLToPath(import.meta.url));
const websiteRoot = path.resolve(scriptDirectory, '..');
const repositoryRoot = path.resolve(websiteRoot, '..');
const docsSourceDirectory = path.join(websiteRoot, 'src/content/docs');
const distDirectory = path.join(websiteRoot, 'dist');
const packageManifestPath = path.join(repositoryRoot, 'packages/cribra/package.json');
const bindingsDataSourcePath = path.join(websiteRoot, 'src/data/bindings.ts');
const docsNavigationPath = path.join(websiteRoot, 'src/pages/docs/index.astro');

const typescriptDocs = new Map([
  ['typescript', '/docs/typescript/'],
  ['typescript-api-reference', '/docs/typescript-api-reference/'],
  [
    'typescript-sessions-and-transformations',
    '/docs/typescript-sessions-and-transformations/',
  ],
  [
    'typescript-runtime-compatibility',
    '/docs/typescript-runtime-compatibility/',
  ],
]);

const failures = [];
const warnings = [];

function fail(message) {
  failures.push(message);
}

function warn(message) {
  warnings.push(message);
}

function assert(condition, message) {
  if (!condition) fail(message);
}

async function pathKind(target) {
  const details = await stat(target).catch(() => null);
  if (!details) return null;
  if (details.isDirectory()) return 'directory';
  if (details.isFile()) return 'file';
  return 'other';
}

async function readText(target) {
  return readFile(target, 'utf8');
}

function decodeHtmlEntities(value) {
  return value
    .replaceAll('&amp;', '&')
    .replaceAll('&lt;', '<')
    .replaceAll('&gt;', '>')
    .replaceAll('&quot;', '"')
    .replaceAll('&#39;', "'")
    .replaceAll('&#x27;', "'");
}

function parseFrontmatter(contents, file) {
  const match = /^---\r?\n([\s\S]*?)\r?\n---\r?\n?/.exec(contents);
  if (!match) {
    fail(`${file}: missing frontmatter block`);
    return null;
  }

  const data = {};
  for (const rawLine of match[1].split(/\r?\n/)) {
    const line = rawLine.trim();
    if (!line || line.startsWith('#')) continue;
    const separator = line.indexOf(':');
    if (separator === -1) {
      fail(`${file}: invalid frontmatter line: ${rawLine}`);
      continue;
    }
    const key = line.slice(0, separator).trim();
    let value = line.slice(separator + 1).trim();
    if (
      (value.startsWith('"') && value.endsWith('"')) ||
      (value.startsWith("'") && value.endsWith("'"))
    ) {
      value = value.slice(1, -1);
    }
    data[key] = value;
  }

  return data;
}

async function loadDocs() {
  const entries = await readdir(docsSourceDirectory, { withFileTypes: true });
  const docs = [];

  for (const entry of entries.sort((a, b) => a.name.localeCompare(b.name))) {
    if (!entry.isFile() || !entry.name.endsWith('.md')) continue;

    const id = entry.name.slice(0, -'.md'.length);
    const file = path.join(docsSourceDirectory, entry.name);
    const contents = await readText(file);
    const data = parseFrontmatter(contents, file);
    if (!data) continue;

    const title = data.title ?? '';
    const description = data.description ?? '';
    const order = Number(data.order);

    if (!title.trim()) fail(`${file}: empty title`);
    if (!description.trim()) fail(`${file}: empty description`);
    if (!Number.isInteger(order)) {
      fail(`${file}: order must be an integer, received ${JSON.stringify(data.order)}`);
    }

    docs.push({ id, file, contents, title, description, order });
  }

  const orders = new Map();
  for (const doc of docs) {
    if (!Number.isInteger(doc.order)) continue;
    if (orders.has(doc.order)) {
      fail(
        `duplicate documentation order ${doc.order}: ${orders.get(doc.order)} and ${doc.id}`,
      );
    }
    orders.set(doc.order, doc.id);
  }

  return docs;
}

function assertHtmlMetadata(doc, html) {
  const titleMatch = /<title>([\s\S]*?)<\/title>/i.exec(html);
  if (!titleMatch) {
    fail(`${doc.id}: rendered page has no <title>`);
  } else if (!decodeHtmlEntities(titleMatch[1]).includes(doc.title)) {
    fail(`${doc.id}: <title> does not contain frontmatter title ${JSON.stringify(doc.title)}`);
  }

  const descriptions = [
    ...html.matchAll(/<meta\s+name="description"\s+content="([^"]*)"/gi),
  ].map((match) => decodeHtmlEntities(match[1]));
  if (descriptions.length !== 1) {
    fail(`${doc.id}: expected exactly one meta description, found ${descriptions.length}`);
  } else if (descriptions[0] !== doc.description) {
    fail(
      `${doc.id}: meta description does not match frontmatter description\n  frontmatter: ${JSON.stringify(doc.description)}\n  rendered:    ${JSON.stringify(descriptions[0])}`,
    );
  }

  assert(
    html.includes('data-pagefind-body'),
    `${doc.id}: rendered page is missing data-pagefind-body`,
  );
}

function extractDocumentLinks(html) {
  const links = [];
  for (const match of html.matchAll(/<a\s[^>]*href="([^"]*)"/gi)) {
    links.push(decodeHtmlEntities(match[1]));
  }
  return links;
}

const browserManagedScheme = /^[a-z][a-z0-9+.-]*:/i;

async function resolveInternalLink(href) {
  if (!href || href.startsWith('//') || browserManagedScheme.test(href)) {
    return { kind: 'external' };
  }

  const [withoutFragment] = href.split('#');
  const [pathname] = withoutFragment.split('?');

  if (!pathname) {
    return { kind: 'fragment-only', fragment: href.split('#')[1] ?? '' };
  }
  if (!pathname.startsWith('/')) {
    return { kind: 'external' };
  }

  let decoded;
  try {
    decoded = decodeURIComponent(pathname);
  } catch {
    decoded = pathname;
  }

  const relative = decoded.replace(/^\/+/, '');
  const absolute = path.resolve(distDirectory, relative);
  if (
    absolute !== distDirectory &&
    !absolute.startsWith(`${distDirectory}${path.sep}`)
  ) {
    return { kind: 'missing', target: decoded };
  }

  const directKind = await pathKind(absolute);
  if (directKind === 'file') {
    return {
      kind: 'file',
      target: absolute,
      fragment: href.includes('#') ? href.split('#')[1] : '',
    };
  }
  if (directKind === 'directory') {
    const indexPath = path.join(absolute, 'index.html');
    if ((await pathKind(indexPath)) === 'file') {
      return {
        kind: 'file',
        target: indexPath,
        fragment: href.includes('#') ? href.split('#')[1] : '',
      };
    }
  }

  return { kind: 'missing', target: decoded };
}

function hasAnchor(contents, fragment) {
  if (!fragment) return true;
  let decoded;
  try {
    decoded = decodeURIComponent(fragment);
  } catch {
    decoded = fragment;
  }
  const escaped = decoded.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  return new RegExp(`(?:id|name)=["']${escaped}["']`).test(contents);
}

async function assertDocumentLinks(doc, html) {
  const links = extractDocumentLinks(html);
  const seen = new Set();

  for (const href of links) {
    if (seen.has(href)) continue;
    seen.add(href);

    const resolved = await resolveInternalLink(href);
    if (resolved.kind === 'external') continue;

    if (resolved.kind === 'missing') {
      // The Swift-DocC tree is staged separately from the Astro build. Skip those
      // links only when the generated tree is absent, rather than pretending it
      // was validated.
      if (resolved.target.startsWith('/api/')) {
        const apiPresent = (await pathKind(path.join(distDirectory, 'api'))) !== null;
        if (!apiPresent) {
          warn(`${doc.id}: skipping unstaged DocC link ${href}`);
          continue;
        }
      }
      fail(`${doc.id}: internal link does not resolve: ${href}`);
      continue;
    }

    if (resolved.kind === 'fragment-only') {
      if (!hasAnchor(html, resolved.fragment)) {
        fail(`${doc.id}: missing local anchor for ${href}`);
      }
      continue;
    }

    if (resolved.kind === 'file' && resolved.fragment) {
      const targetContents = await readText(resolved.target).catch(() => '');
      if (!hasAnchor(targetContents, resolved.fragment)) {
        fail(`${doc.id}: link fragment does not exist: ${href}`);
      }
    }
  }
}

async function loadPagefindUrls() {
  const fragmentDirectory = path.join(distDirectory, 'pagefind/fragment');
  const details = await stat(fragmentDirectory).catch(() => null);
  if (!details?.isDirectory()) {
    fail('Pagefind fragment directory is missing; run npm run build');
    return new Set();
  }

  const urls = new Set();
  for (const file of await readdir(fragmentDirectory)) {
    if (!file.endsWith('.pf_fragment')) continue;
    const compressed = await readFile(path.join(fragmentDirectory, file));
    let payload;
    try {
      payload = gunzipSync(compressed).toString('utf8');
    } catch (error) {
      fail(`Pagefind fragment is not gzip-compressed: ${file} (${error.message})`);
      continue;
    }

    // Pagefind fragments are a short codec marker followed by JSON metadata.
    const decoded = payload.replace(/^pagefind_[a-z0-9]+/, '');
    try {
      const metadata = JSON.parse(decoded);
      if (typeof metadata.url === 'string') urls.add(metadata.url);
    } catch (error) {
      fail(`Pagefind fragment metadata is not JSON: ${file} (${error.message})`);
    }
  }

  return urls;
}

async function loadSitemapUrls(requireSitemap) {
  const indexPath = path.join(distDirectory, 'sitemap-index.xml');
  const indexContents = await readText(indexPath).catch(() => null);

  if (indexContents === null) {
    if (requireSitemap) {
      fail('sitemap-index.xml is missing; build with ASTRO_SITE_URL to validate coverage');
    } else {
      warn('sitemap not generated because ASTRO_SITE_URL is unset; skipping sitemap coverage');
    }
    return null;
  }

  const sitemapFiles = [...indexContents.matchAll(/<loc>([^<]+)<\/loc>/g)].map(
    (match) => match[1],
  );
  if (sitemapFiles.length === 0) {
    fail('sitemap-index.xml contains no sitemap entries');
    return null;
  }

  const urls = new Set();
  for (const sitemapUrl of sitemapFiles) {
    const filename = new URL(sitemapUrl).pathname.split('/').pop();
    const contents = await readText(path.join(distDirectory, filename)).catch(() => null);
    if (contents === null) {
      fail(`sitemap entry points at a missing file: ${filename}`);
      continue;
    }
    for (const match of contents.matchAll(/<loc>([^<]+)<\/loc>/g)) {
      urls.add(match[1]);
    }
  }

  if (urls.size === 0) {
    fail('sitemap files contain no page URLs');
  }

  return urls;
}

async function checkBindingsData() {
  const contents = await readText(bindingsDataSourcePath);
  assert(contents.includes("id: 'typescript'"), 'bindings data has no TypeScript entry');
  assert(
    contents.includes('cribra-typescript 0.1.0'),
    'bindings data does not record cribra-typescript 0.1.0',
  );
  assert(
    contents.includes('provisional and unpublished'),
    'bindings data does not mark the TypeScript package as provisional and unpublished',
  );
  assert(
    contents.includes('TypeScript native backend'),
    'bindings data no longer retains the planned native TypeScript backend',
  );
  assert(
    !contents.includes('TypeScript isomorphic API contract'),
    'bindings data still lists the completed TypeScript API contract as planned',
  );
  assert(
    !contents.includes('TypeScript / WebAssembly wrapper'),
    'bindings data still lists the implemented WebAssembly wrapper as planned',
  );

  const navigation = await readText(docsNavigationPath);
  for (const url of typescriptDocs.values()) {
    assert(
      navigation.includes(`'${url}'`),
      `docs navigation does not link ${url}`,
    );
  }
}

async function checkPackageIdentity() {
  const manifest = JSON.parse(await readText(packageManifestPath));
  assert(manifest.name === 'cribra-typescript', 'package name must remain cribra-typescript');
  assert(manifest.version === '0.1.0', 'package version must remain 0.1.0');
  assert(manifest.private === true, 'package must remain private');
  assert(
    manifest.cribraRelease?.state === 'pending-ownership',
    'cribraRelease.state must remain pending-ownership',
  );
  assert(
    manifest.cribraRelease?.approvedName === '@metalymph/cribra',
    'approved future package name must remain @metalymph/cribra',
  );
  assert(manifest.engines?.node === '>=22', 'engines.node must remain >=22');
}

async function walkFiles(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const files = [];
  for (const entry of entries) {
    const target = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      if (['node_modules', 'dist', '.astro', 'target'].includes(entry.name)) continue;
      files.push(...(await walkFiles(target)));
    } else if (entry.isFile() && /\.(?:md|mdx|astro|ts|mjs)$/.test(entry.name)) {
      files.push(target);
    }
  }
  return files;
}

async function checkForbiddenInstallCommands() {
  const roots = [
    path.join(websiteRoot, 'src'),
    path.join(websiteRoot, 'README.md'),
    path.join(repositoryRoot, 'README.md'),
    path.join(repositoryRoot, 'ROADMAP.md'),
    path.join(repositoryRoot, 'docs'),
    path.join(repositoryRoot, 'packages/cribra/README.md'),
  ];

  const installCommand =
    /(?:npm\s+(?:install|i|add)|pnpm\s+(?:add|install)|yarn\s+add|bun\s+add)[^\n]*@metalymph\/cribra/;

  for (const root of roots) {
    const kind = await pathKind(root);
    if (kind === null) {
      fail(`publication-boundary scan target is missing: ${root}`);
      continue;
    }
    const files = kind === 'file' ? [root] : await walkFiles(root);
    for (const file of files) {
      const contents = await readText(file);
      const match = installCommand.exec(contents);
      if (match) {
        fail(
          `executable install instruction for the unapproved package name in ${path.relative(repositoryRoot, file)}: ${match[0]}`,
        );
      }
    }
  }
}

async function checkTypescriptSnippetCoverage() {
  const fixturePath = path.join(
    repositoryRoot,
    'packages/cribra/qualification/apps/typescript/docs-examples.ts',
  );
  const fixture = await readText(fixturePath).catch(() => null);
  if (fixture === null) {
    fail('missing installed-tarball TypeScript documentation fixture');
    return;
  }

  const markers = new Set(
    [...fixture.matchAll(/docs-example:\s*([^\s*]+)/g)].map((match) => match[1]),
  );
  const titles = new Set();

  for (const id of typescriptDocs.keys()) {
    const file = path.join(docsSourceDirectory, `${id}.md`);
    const contents = await readText(file).catch(() => null);
    if (contents === null) continue;

    for (const match of contents.matchAll(/```ts(?: title="([^"]*)")?\r?\n([\s\S]*?)```/g)) {
      const [, title = '', code] = match;
      if (title) titles.add(title);

      for (const forbidden of [
        '@metalymph/cribra',
        'packages/cribra/src',
        'packages/cribra/dist',
        '../src/',
        '../dist/',
        '/dist/',
      ]) {
        assert(
          !code.includes(forbidden),
          `${id}.md: TypeScript example uses forbidden import or path ${forbidden}`,
        );
      }

      const publicOperations = [
        'init',
        'isInitialized',
        'createScanner',
        'getCapabilities',
        'getVersion',
        'byteSpanToTextRange',
        'createShareBundle',
        'scan',
        'scanBatch',
        'stream',
        'incremental',
        'transform',
        'push',
        'finish',
        'applyEdit',
        'snapshot',
        'dispose',
      ];
      for (const operation of publicOperations) {
        if (!new RegExp(`\\b${operation}\\s*\\(`).test(code)) continue;
        assert(
          new RegExp(`\\b${operation}\\s*\\(`).test(fixture),
          `${id}.md: operation ${operation}() has no executable coverage in docs-examples.ts`,
        );
      }
    }
  }

  for (const title of titles) {
    assert(
      markers.has(title),
      `${title}: published example has no executable coverage marker in docs-examples.ts`,
    );
  }
  for (const marker of markers) {
    assert(
      titles.has(marker),
      `${marker}: docs-examples.ts marker has no matching published fenced example`,
    );
  }

  const importedNames = (source) => {
    const names = new Set();
    for (const match of source.matchAll(
      /import\s+(?:type\s+)?\{([\s\S]*?)\}\s+from\s+["'](?:cribra-typescript|__CRIBRA_PACKAGE_SPECIFIER__)["']/g,
    )) {
      for (const raw of match[1].split(',')) {
        const name = raw.trim().replace(/^type\s+/, '');
        if (name) names.add(name);
      }
    }
    return names;
  };

  const fixtureImports = importedNames(fixture);
  const documentedImports = new Set();
  for (const id of typescriptDocs.keys()) {
    const contents = await readText(path.join(docsSourceDirectory, `${id}.md`));
    for (const name of importedNames(contents)) documentedImports.add(name);
  }

  for (const name of documentedImports) {
    assert(
      fixtureImports.has(name),
      `${name}: imported by documentation but absent from the installed-tarball fixture`,
    );
  }
  for (const name of fixtureImports) {
    assert(
      documentedImports.has(name),
      `${name}: exercised by docs-examples.ts but absent from the published examples`,
    );
  }
}

function checkTypescriptDocsStatus(docsById) {
  for (const id of typescriptDocs.keys()) {
    assert(docsById.has(id), `missing TypeScript documentation page: ${id}.md`);
  }

  const quickStart = docsById.get('typescript');
  if (quickStart) {
    assert(
      /not published/i.test(quickStart.contents),
      'TypeScript quick start must state that the package is not published',
    );
    assert(
      /pending-ownership/.test(quickStart.contents),
      'TypeScript quick start must record cribraRelease.state pending-ownership',
    );
  }

  const runtime = docsById.get('typescript-runtime-compatibility');
  if (runtime) {
    assert(
      /not published/i.test(runtime.contents),
      'TypeScript runtime guide must state that the package is not published',
    );
    assert(
      /Native Safari[^.]*unverified/i.test(runtime.contents) ||
        /unverified[^.]*Native Safari/i.test(runtime.contents),
      'TypeScript runtime guide must keep native Safari unverified',
    );
    assert(
      /React Native/.test(runtime.contents) && /Not claimed/i.test(runtime.contents),
      'TypeScript runtime guide must exclude React Native from claims',
    );
    assert(
      /multi-threaded/i.test(runtime.contents),
      'TypeScript runtime guide must exclude multi-threaded WASM from claims',
    );
  }
}

async function main() {
  const requireSitemap = process.argv.includes('--require-sitemap');

  const distKind = await pathKind(distDirectory);
  if (distKind !== 'directory') {
    fail('missing built website: website/dist; run npm run build');
  } else {
    const docs = await loadDocs();
    const docsById = new Map(docs.map((doc) => [doc.id, doc]));
    checkTypescriptDocsStatus(docsById);
    await checkTypescriptSnippetCoverage();

    for (const doc of docs) {
      const htmlPath = path.join(distDirectory, 'docs', doc.id, 'index.html');
      const html = await readText(htmlPath).catch(() => null);
      if (html === null) {
        fail(`${doc.id}: missing rendered route /docs/${doc.id}/`);
        continue;
      }
      assertHtmlMetadata(doc, html);
      await assertDocumentLinks(doc, html);
    }

    const pagefindUrls = await loadPagefindUrls();
    for (const doc of docs) {
      const url = `/docs/${doc.id}/`;
      assert(pagefindUrls.has(url), `${doc.id}: route is absent from the Pagefind index (${url})`);
    }
    for (const [id, url] of typescriptDocs) {
      assert(pagefindUrls.has(url), `${id}: new TypeScript route is absent from the Pagefind index (${url})`);
    }

    const sitemapUrls = await loadSitemapUrls(requireSitemap);
    if (sitemapUrls) {
      const origin = new URL(sitemapUrls.values().next().value).origin;
      for (const doc of docs) {
        const url = `/docs/${doc.id}/`;
        const canonical = await readMetadataCanonical(doc.id);
        if (canonical === null) {
          fail(`${doc.id}: missing canonical URL in sitemap-enabled build`);
          continue;
        }
        assert(
          canonical === `${origin}${url}`,
          `${doc.id}: canonical URL must be ${origin}${url}, received ${canonical}`,
        );
        assert(
          sitemapUrls.has(canonical),
          `${doc.id}: route is absent from the sitemap (${canonical})`,
        );
      }
    }

    await checkBindingsData();
    await checkPackageIdentity();
    await checkForbiddenInstallCommands();
  }

  for (const warning of warnings) {
    console.warn(`WARN: ${warning}`);
  }

  if (failures.length > 0) {
    for (const failure of failures) {
      console.error(`FAIL: ${failure}`);
    }
    console.error(`\n${failures.length} content validation failure(s).`);
    process.exit(1);
  }

  console.log('Content validation passed.');
}

async function readMetadataCanonical(id) {
  const html = await readText(path.join(distDirectory, 'docs', id, 'index.html')).catch(
    () => null,
  );
  if (html === null) return null;
  const match = /<link\s+rel="canonical"\s+href="([^"]*)"/i.exec(html);
  return match ? decodeHtmlEntities(match[1]) : null;
}

await main();
