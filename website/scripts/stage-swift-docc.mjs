#!/usr/bin/env node

import {
  cp,
  mkdir,
  readFile,
  readdir,
  rm,
  stat,
  writeFile,
} from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const scriptDirectory = path.dirname(fileURLToPath(import.meta.url));
const repositoryRoot = path.resolve(scriptDirectory, '../..');

const requiredSymbols = [
  'scanner',
  'scanreport',
  'finding',
  'streamsession',
  'incrementalsession',
];

function fail(message) {
  console.error(`FAIL: ${message}`);
  process.exit(1);
}

function parseArguments(argv) {
  const options = {
    profile: 'core',
    version: '0.7.2',
  };

  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];

    if (argument === '--profile' || argument === '--version') {
      const value = argv[index + 1];
      if (!value) {
        fail(`${argument} requires a value`);
      }
      options[argument.slice(2)] = value;
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

  return files;
}

function assertHostedHtml(file, contents, basePath) {
  const marker = `var baseUrl = "${basePath}/"`;
  if (!contents.includes(marker)) {
    fail(`${path.relative(repositoryRoot, file)} does not use ${basePath}/`);
  }

  const rootReference = /\b(?:src|href)="(\/[^"]*)"/g;
  for (const match of contents.matchAll(rootReference)) {
    const reference = match[1];
    if (reference.startsWith('//')) {
      continue;
    }
    if (reference === basePath || reference.startsWith(`${basePath}/`)) {
      continue;
    }

    fail(
      `${path.relative(repositoryRoot, file)} contains root-relative reference ${reference}`,
    );
  }
}

const options = parseArguments(process.argv.slice(2));

if (options.profile !== 'core') {
  fail(`only the core profile is supported in W2.9.1: ${options.profile}`);
}

if (!/^\d+\.\d+\.\d+$/.test(options.version)) {
  fail(`version must be a semantic version without a prefix: ${options.version}`);
}

const basePath = `/api/${options.version}/swift/${options.profile}`;
const source = path.join(
  repositoryRoot,
  'target',
  'swift-docc',
  options.profile,
  'Cribra.doccarchive',
);
const destination = path.join(
  repositoryRoot,
  'website',
  'public',
  'api',
  options.version,
  'swift',
  options.profile,
);
const moduleIndex = path.join(
  source,
  'documentation',
  'cribra',
  'index.html',
);
const rootRedirect = `${basePath}/documentation/cribra/`;

const sourceStats = await stat(source).catch(() => null);
if (!sourceStats?.isDirectory()) {
  fail(
    `missing DocC archive: ${path.relative(repositoryRoot, source)}; run just swift-docc-core`,
  );
}

const moduleHtml = await readFile(moduleIndex, 'utf8').catch(() => null);
if (moduleHtml === null) {
  fail(`missing DocC module page: ${path.relative(repositoryRoot, moduleIndex)}`);
}

const sourceFiles = await walk(source);
const sourceHtmlFiles = sourceFiles.filter((file) => file.endsWith('.html'));
if (sourceHtmlFiles.length === 0) {
  fail('DocC archive contains no HTML files');
}

for (const file of sourceHtmlFiles) {
  assertHostedHtml(file, await readFile(file, 'utf8'), basePath);
}

for (const symbol of requiredSymbols) {
  const symbolIndex = path.join(
    source,
    'documentation',
    'cribra',
    symbol,
    'index.html',
  );
  const symbolStats = await stat(symbolIndex).catch(() => null);
  if (!symbolStats?.isFile()) {
    fail(
      `missing required symbol page: ${path.relative(repositoryRoot, symbolIndex)}`,
    );
  }
}

await rm(destination, { recursive: true, force: true });
await mkdir(path.dirname(destination), { recursive: true });
await cp(source, destination, { recursive: true });

const redirectHtml = `<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <meta http-equiv="refresh" content="0; url=${rootRedirect}">
    <link rel="canonical" href="${rootRedirect}">
    <title>Cribra Swift API reference</title>
  </head>
  <body>
    <p><a href="${rootRedirect}">Cribra Swift API reference</a></p>
    <script>window.location.replace(${JSON.stringify(rootRedirect)});</script>
  </body>
</html>
`;

await writeFile(path.join(destination, 'index.html'), redirectHtml, 'utf8');

const destinationFiles = await walk(destination);
const destinationBytes = (
  await Promise.all(
    destinationFiles.map(async (file) => (await stat(file)).size),
  )
).reduce((total, size) => total + size, 0);

for (const file of destinationFiles.filter((entry) => entry.endsWith('.html'))) {
  const contents = await readFile(file, 'utf8');
  if (path.basename(file) === 'index.html' && path.dirname(file) === destination) {
    continue;
  }
  assertHostedHtml(file, contents, basePath);
}

console.log(`PASS: hosted base path ${basePath}`);
console.log(
  `PASS: staged ${destinationFiles.length} files (${(destinationBytes / 1024 / 1024).toFixed(1)} MiB) at ${path.relative(repositoryRoot, destination)}`,
);
console.log(`PASS: entry point redirects to ${rootRedirect}`);
