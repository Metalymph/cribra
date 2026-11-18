#!/usr/bin/env node

import {
  mkdir,
  readdir,
  readFile,
  stat,
  writeFile,
} from 'node:fs/promises';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const scriptDirectory = path.dirname(fileURLToPath(import.meta.url));
const repositoryRoot = path.resolve(scriptDirectory, '../..');
const distDirectory = path.join(repositoryRoot, 'website', 'dist');
const apiRelative = 'api/0.7.2/swift/core';
const apiBasePath = `/${apiRelative}`;
const apiDirectory = path.join(distDirectory, apiRelative);

const requiredSymbols = [
  'scanner',
  'scanreport',
  'finding',
  'streamsession',
  'incrementalsession',
];

const browserManagedSchemes = /^[a-z][a-z0-9+.-]*:/i;
const edgeManagedPrefixes = ['/cdn-cgi/'];

function fail(message) {
  console.error(`FAIL: ${message}`);
  process.exit(1);
}

function parseArguments(argv) {
  const options = { manifest: null };

  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];

    if (argument === '--manifest') {
      const value = argv[index + 1];
      if (!value) {
        fail('--manifest requires a path');
      }
      options.manifest = path.resolve(value);
      index += 1;
      continue;
    }

    fail(`unknown argument: ${argument}`);
  }

  return options;
}

async function walk(directory) {
  const entries = await readdir(directory, { withFileTypes: true });
  const files = [];

  for (const entry of entries) {
    const entryPath = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      files.push(...(await walk(entryPath)));
    } else if (entry.isFile()) {
      files.push(entryPath);
    }
  }

  return files.sort();
}

function relativeToDist(absolutePath) {
  return path.relative(distDirectory, absolutePath).split(path.sep).join('/');
}

async function assertFile(relativePath, description) {
  const absolutePath = path.join(distDirectory, relativePath);
  const details = await stat(absolutePath).catch(() => null);

  if (!details?.isFile()) {
    fail(`missing ${description}: ${relativePath}`);
  }
  if (details.size === 0) {
    fail(`empty ${description}: ${relativePath}`);
  }

  return details;
}

async function assertJson(relativePath, description) {
  const absolutePath = path.join(distDirectory, relativePath);
  const contents = await readFile(absolutePath, 'utf8').catch(() => null);

  if (contents === null) {
    fail(`missing ${description}: ${relativePath}`);
  }

  try {
    JSON.parse(contents);
  } catch (error) {
    fail(`invalid JSON in ${description}: ${relativePath} (${error.message})`);
  }
}

async function resolvesInsideDist(urlPath) {
  const relativePath = urlPath.replace(/^\/+/, '');
  const absolutePath = path.resolve(distDirectory, relativePath);

  if (
    absolutePath !== distDirectory &&
    !absolutePath.startsWith(`${distDirectory}${path.sep}`)
  ) {
    return false;
  }

  const direct = await stat(absolutePath).catch(() => null);
  if (direct?.isFile()) {
    return true;
  }

  // Astro's trailingSlash output and DocC's static routes both use index.html.
  const index = await stat(path.join(absolutePath, 'index.html')).catch(
    () => null,
  );
  return Boolean(index?.isFile());
}

function normalizeReference(rawReference) {
  // Fragment-only and query-only links are managed by the browser.
  if (
    !rawReference ||
    rawReference.startsWith('#') ||
    rawReference.startsWith('?')
  ) {
    return null;
  }

  // External and protocol-relative URLs are outside the static output tree.
  if (browserManagedSchemes.test(rawReference)) {
    return null;
  }
  if (!rawReference.startsWith('/') || rawReference.startsWith('//')) {
    return null;
  }

  // Cloudflare may inject /cdn-cgi/... links at the edge.
  if (edgeManagedPrefixes.some((prefix) => rawReference.startsWith(prefix))) {
    return null;
  }

  const withoutQueryOrFragment = rawReference.split('#')[0].split('?')[0];
  if (!withoutQueryOrFragment) {
    return null;
  }

  let decoded;
  try {
    decoded = decodeURIComponent(withoutQueryOrFragment);
  } catch {
    decoded = withoutQueryOrFragment;
  }

  return path.posix.normalize(decoded);
}

const options = parseArguments(process.argv.slice(2));

const distDetails = await stat(distDirectory).catch(() => null);
if (!distDetails?.isDirectory()) {
  fail('missing built website: website/dist; run just website-build');
}

const requiredFiles = [
  ['index.html', 'Astro homepage'],
  [`${apiRelative}/index.html`, 'Swift-DocC entry point'],
  [
    `${apiRelative}/documentation/cribra/index.html`,
    'Swift-DocC module page',
  ],
  [`${apiRelative}/data/documentation/cribra.json`, 'DocC module data'],
  [`${apiRelative}/index/index.json`, 'DocC search index'],
  [`${apiRelative}/theme-settings.json`, 'DocC theme settings'],
  [`${apiRelative}/favicon.svg`, 'DocC SVG favicon'],
  [`${apiRelative}/favicon.ico`, 'DocC favicon'],
  ['pagefind/pagefind.js', 'Pagefind runtime'],
  ['pagefind/pagefind-entry.json', 'Pagefind entry metadata'],
];

const requiredFileSizes = {};
for (const [relativePath, description] of requiredFiles) {
  requiredFileSizes[relativePath] = (await assertFile(relativePath, description))
    .size;
}

for (const [relativePath, description] of [
  [`${apiRelative}/data/documentation/cribra.json`, 'DocC module data'],
  [`${apiRelative}/index/index.json`, 'DocC search index'],
  [`${apiRelative}/theme-settings.json`, 'DocC theme settings'],
  ['pagefind/pagefind-entry.json', 'Pagefind entry metadata'],
]) {
  await assertJson(relativePath, description);
}

for (const symbol of requiredSymbols) {
  await assertFile(
    `${apiRelative}/documentation/cribra/${symbol}/index.html`,
    `DocC symbol page for ${symbol}`,
  );
}

const entryHtml = await readFile(
  path.join(distDirectory, apiRelative, 'index.html'),
  'utf8',
);
const expectedModulePath = `${apiBasePath}/documentation/cribra/`;
if (!entryHtml.includes(`url=${expectedModulePath}`)) {
  fail(`Swift-DocC entry point does not redirect to ${expectedModulePath}`);
}
if (!entryHtml.includes(`href="${expectedModulePath}"`)) {
  fail(`Swift-DocC entry point is missing its canonical link`);
}

const allFiles = await walk(distDirectory);
const htmlFiles = allFiles.filter((file) => file.endsWith('.html'));
if (htmlFiles.length === 0) {
  fail('website/dist contains no HTML files');
}

const doccFiles = allFiles.filter((file) =>
  file.startsWith(`${apiDirectory}${path.sep}`),
);
const doccCssFiles = doccFiles.filter(
  (file) => file.endsWith('.css') && file.includes(`${path.sep}css${path.sep}`),
);
const doccJsFiles = doccFiles.filter(
  (file) => file.endsWith('.js') && file.includes(`${path.sep}js${path.sep}`),
);
if (doccCssFiles.length === 0) {
  fail('DocC output contains no CSS assets');
}
if (doccJsFiles.length === 0) {
  fail('DocC output contains no JavaScript assets');
}

const pagefindDirectory = path.join(distDirectory, 'pagefind');
const pagefindFiles = allFiles.filter((file) =>
  file.startsWith(`${pagefindDirectory}${path.sep}`),
);
const pagefindFragments = pagefindFiles.filter((file) =>
  file.endsWith('.pf_fragment'),
);
if (pagefindFragments.length === 0) {
  fail('Pagefind output contains no index fragments');
}

let referenceOccurrences = 0;
const uniqueReferences = new Set();
const brokenReferences = new Map();

for (const htmlFile of htmlFiles) {
  const html = await readFile(htmlFile, 'utf8');

  for (const match of html.matchAll(/(?:href|src)="([^"]*)"/g)) {
    const normalized = normalizeReference(match[1].trim());
    if (normalized === null) {
      continue;
    }

    referenceOccurrences += 1;
    uniqueReferences.add(normalized);

    if (!(await resolvesInsideDist(normalized))) {
      if (!brokenReferences.has(normalized)) {
        brokenReferences.set(normalized, new Set());
      }
      brokenReferences.get(normalized).add(relativeToDist(htmlFile));
    }
  }
}

if (brokenReferences.size > 0) {
  const details = [...brokenReferences.entries()]
    .slice(0, 25)
    .map(([reference, sources]) => {
      const examples = [...sources].slice(0, 3).join(', ');
      return `  ${reference} <- ${examples}`;
    })
    .join('\n');
  const remainder =
    brokenReferences.size > 25
      ? `\n  ... and ${brokenReferences.size - 25} more`
      : '';

  fail(
    `found ${brokenReferences.size} broken root-relative references:\n${details}${remainder}`,
  );
}

const totalBytes = (
  await Promise.all(allFiles.map(async (file) => (await stat(file)).size))
).reduce((total, size) => total + size, 0);

const manifest = {
  dist: 'website/dist',
  totalFiles: allFiles.length,
  totalBytes,
  htmlFiles: htmlFiles.length,
  doccFiles: doccFiles.length,
  pagefindFiles: pagefindFiles.length,
  pagefindFragments: pagefindFragments.length,
  rootRelativeReferenceOccurrences: referenceOccurrences,
  uniqueRootRelativeReferences: uniqueReferences.size,
  requiredFiles: requiredFileSizes,
};

console.log('PASS: Astro homepage and Swift-DocC entry point');
console.log(
  `PASS: DocC module page and ${requiredSymbols.length} required symbol pages`,
);
console.log(
  `PASS: ${doccCssFiles.length} DocC CSS assets and ${doccJsFiles.length} DocC JavaScript assets`,
);
console.log(
  `PASS: Pagefind index with ${pagefindFragments.length} fragments`,
);
console.log(
  `PASS: ${referenceOccurrences} root-relative references (${uniqueReferences.size} unique) resolved across ${htmlFiles.length} HTML files`,
);
console.log(
  `PASS: ${allFiles.length} files (${(totalBytes / 1024 / 1024).toFixed(1)} MiB) in website/dist`,
);

if (options.manifest) {
  await mkdir(path.dirname(options.manifest), { recursive: true });
  await writeFile(
    options.manifest,
    `${JSON.stringify(manifest, null, 2)}\n`,
    'utf8',
  );
  console.log(
    `PASS: manifest written to ${path.relative(repositoryRoot, options.manifest)}`,
  );
}
