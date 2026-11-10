const path = require('path');
const { getDefaultConfig, mergeConfig } = require('@react-native/metro-config');

/**
 * The private Cribra package is linked from the parent directory. Metro needs
 * the package root in its watch folders so package.json, the Codegen spec and
 * the TypeScript facade resolve through the local npm link, and it must resolve
 * the package's Babel/React dependencies from the consumer's node_modules.
 *
 * The nested consumer is the Metro project root itself, so it is not crawled
 * twice.
 *
 * @type {import('@react-native/metro-config').MetroConfig}
 */
const config = {
  watchFolders: [path.resolve(__dirname, '..')],
  resolver: {
    nodeModulesPaths: [path.resolve(__dirname, 'node_modules')],
    extraNodeModules: {
      '@metalymph/cribra-react-native': path.resolve(__dirname, '..'),
    },
  },
};

module.exports = mergeConfig(getDefaultConfig(__dirname), config);
