import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { chmodSync, cpSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, statSync, utimesSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const source = fileURLToPath(new URL('../', import.meta.url));
const output = resolve(process.argv[2] ?? 'dist');
if (!process.argv[3]) throw new Error('Pass the path to React Native 0.86.3 as the second argument');
const rn = resolve(process.argv[3]);
const codegen = resolve(rn, '../@react-native/codegen');
const packageVersion = (dir) => JSON.parse(readFileSync(join(dir, 'package.json'), 'utf8')).version;
if (packageVersion(rn) !== '0.86.3' || packageVersion(codegen) !== '0.86.3') {
  throw new Error('Use the locked React Native and Codegen 0.86.3 toolchain');
}
const root = mkdtempSync(join(tmpdir(), 'cribra-rn86-'));
const candidate = join(root, 'package');
const name = 'cribra-react-native-rn86';
const version = '0.1.0';

function project(file, edits) {
  const path = join(candidate, file);
  let contents = readFileSync(path, 'utf8');
  for (const [start, end] of edits) {
    const from = contents.indexOf(start);
    const to = contents.indexOf(end, from + start.length);
    if (from < 0 || to < 0 || contents.indexOf(start, from + start.length) >= 0) {
      throw new Error('Review the RN 0.86 projection for ' + file + ': ' + start);
    }
    contents = contents.slice(0, from) + contents.slice(to);
  }
  writeFileSync(path, contents);
}

function sha(path) {
  return createHash('sha256').update(readFileSync(path)).digest('hex');
}

function listFiles(directory, relative = '') {
  return readdirSync(directory).sort().flatMap((name) => {
    const location = join(directory, name);
    const path = relative + name;
    return statSync(location).isDirectory() ? listFiles(location, path + '/') : [path];
  });
}

const archive = join(output, name + '.tgz');
try {
  mkdirSync(candidate);
  for (const file of ['src', 'cpp', 'ios/OnLoad.mm', 'ios/CribraCAPI.xcframework',
    'android/CMakeLists.txt', 'android/src/main/cpp/prebuilt', 'third_party/cribra/include',
    'CribraReactNative.podspec', 'react-native.config.js', 'LICENSE']) {
    mkdirSync(resolve(candidate, file, '..'), { recursive: true });
    cpSync(join(source, file), join(candidate, file), { recursive: true });
  }

  project('src/NativeCribraReactNative.ts', [
    ['  /**\n   * One scan-coupled pseudonymization.', '  disposeScanner('],
  ]);
  project('src/index.ts', [
    ['import {\n  copyTransformKey,', 'import type { TransformRequest }'],
    ['  PseudonymizeRequest,\n', '  RedactRequest,\n'],
    ['  SynthesizeRequest,\n', '  TemplateRequest,\n'],
    ['   *\n   * Keyed modes copy', '   */\n  async transform('],
    ['    // Validate the complete request', '    const normalized = normalizeTransformRequest(request);'],
    ["        case 'pseudonymize':", '        default:'],
  ]);
  const index = join(candidate, 'src/index.ts');
  writeFileSync(index, readFileSync(index, 'utf8').replace(
    "import type { TransformRequest } from './transform';",
    "import { normalizeTransformRequest, type TransformRequest } from './transform';",
  ));
  project('src/transform.ts', [
    ['/** Exact key size', 'export interface RedactRequest'],
    ['export interface PseudonymizeRequest', 'function invalidArgument()'],
    ['function invalidKeyLength()', 'function isRecord('],
    ['function requireKey(', 'export function normalizeTransformRequest('],
    ["    case 'pseudonymize':", '    default:'],
    ['\n/**\n * Copies a validated key', '\nexport function zeroTransformKey('],
  ]);
  const transform = join(candidate, 'src/transform.ts');
  const validation = readFileSync(transform, 'utf8');
  writeFileSync(transform, validation.slice(0, validation.indexOf('\nexport function zeroTransformKey('))
    .replace('for redaction, templating, pseudonymization and synthesis.', 'for redaction and templating.')
    .replace('replacement/namespace/prefix/marker', 'replacement/namespace')
    .replace('function invalidArgument()', `const DEFAULT_NAMESPACE = 'CRIBRA';
export type TransformRequest = RedactRequest | TemplateRequest;
type NormalizedTransform = RedactRequest | Required<TemplateRequest>;

function invalidArgument()`));

  project('cpp/CribraReactNative.h', [
    ['  jsi::Value transformPseudonymize(', '  jsi::Value disposeScanner('],
  ]);
  project('cpp/CribraReactNative.cpp', [
    ['/// Copies exactly 32 key bytes', '\n}  // namespace'],
    ['jsi::Value CribraReactNative::transformPseudonymize(', '/// Posts exactly one worker job'],
  ]);

  // Expo autolinks a pure C++ package without a Gradle project to run Codegen.
  // Ship the generated C++ interface with the Android CMake target upstream.
  const schema = join(root, 'schema.json');
  const generated = join(root, 'android-codegen');
  const spec = 'CribraReactNativeSpec';
  execFileSync(process.execPath, [join(codegen, 'lib/cli/combine/combine-js-to-schema-cli.js'),
    schema, join(candidate, 'src/NativeCribraReactNative.ts')]);
  execFileSync(process.execPath, [join(rn, 'scripts/generate-specs-cli.js'),
    '-p', 'android', '-s', schema, '-o', generated, '-n', spec, '-t', 'modules']);
  const header = spec + 'JSI.h';
  mkdirSync(join(candidate, 'android/codegen'));
  const jni = join(generated, 'jni');
  cpSync(join(jni, 'react/renderer/components', spec, header), join(candidate, 'android/codegen', header));
  for (const file of [spec + '.h', spec + '-generated.cpp']) {
    cpSync(join(jni, file), join(candidate, 'android/codegen', file));
  }
  const cmake = join(candidate, 'android/CMakeLists.txt');
  writeFileSync(cmake, readFileSync(cmake, 'utf8').replace('add_library(cribra-react-native STATIC',
    'add_library(react_codegen_CribraReactNativeSpec OBJECT codegen/CribraReactNativeSpec-generated.cpp)\n' +
    'target_include_directories(react_codegen_CribraReactNativeSpec PUBLIC codegen)\n' +
    'target_link_libraries(react_codegen_CribraReactNativeSpec fbjni jsi reactnative)\n' +
    'target_compile_reactnative_options(react_codegen_CribraReactNativeSpec PRIVATE)\n\n' +
    'add_library(cribra-react-native STATIC'));

  const full = JSON.parse(readFileSync(join(source, 'package.json'), 'utf8'));
  const { scripts, files, devDependencies, ...manifest } = full;
  Object.assign(manifest, {
    name: '@metalymph/' + name, version,
    description: 'Cribra native scanner and unkeyed transformations for React Native 0.86',
    peerDependencies: { ...manifest.peerDependencies, 'react-native': '>=0.86.0 <0.87.0' },
  });
  writeFileSync(join(candidate, 'package.json'), JSON.stringify(manifest, null, 2) + '\n');
  writeFileSync(join(candidate, 'README.md'), '# Cribra React Native 0.86\n\nPrivate upstream variant. Supports scan, batch, Redact and Template. Install only one Cribra native package per application.\n');

  const filesInPackage = listFiles(candidate);
  for (const file of filesInPackage) {
    chmodSync(join(candidate, file), 0o644);
    utimesSync(join(candidate, file), 0, 0);
  }
  mkdirSync(output, { recursive: true });
  const tar = join(root, 'package.tar');
  execFileSync('tar', ['--format', 'ustar', '--uid', '0', '--gid', '0', '--uname', 'root',
    '--gname', 'root', '--no-recursion',
    '-cf', tar, '-C', root, ...filesInPackage.map((file) => 'package/' + file)], {
    env: { ...process.env, COPYFILE_DISABLE: '1' },
  });
  const gzip = execFileSync('gzip', ['-n', '-c', tar], { maxBuffer: 100 * 1024 * 1024 });
  writeFileSync(archive, gzip);
  const git = (...args) => execFileSync('git', ['-C', source, ...args], { encoding: 'utf8' }).trim();
  const manifestOutput = {
    package: manifest.name,
    version,
    upstreamCommit: git('rev-parse', 'HEAD'),
    toolchain: {
      rust: execFileSync('rustc', ['--version'], { encoding: 'utf8' }).trim(),
      xcode: execFileSync('xcodebuild', ['-version'], { encoding: 'utf8' }).trim(),
      cargoNdk: execFileSync('cargo', ['ndk', '--version'], { encoding: 'utf8' }).trim(),
      androidNdk: process.env.CRIBRA_NDK_VERSION ?? '27.1.12297006',
      reactNative: packageVersion(rn),
      codegen: packageVersion(codegen),
      node: process.version,
    },
    cargoLockSha256: sha(resolve(source, '../../Cargo.lock')),
    sha256: sha(archive),
    files: Object.fromEntries(filesInPackage.map((file) => [file, sha(join(candidate, file))])),
  };
  writeFileSync(join(output, 'manifest.json'), JSON.stringify(manifestOutput, null, 2) + '\n');
} finally {
  rmSync(root, { recursive: true, force: true });
}
