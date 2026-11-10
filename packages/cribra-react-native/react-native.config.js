/**
 * Pure C++ TurboModule registration. React Native autolinking uses these
 * entries to build the C++ module and register it without a Java/ObjC module.
 *
 * @type {import('@react-native-community/cli-types').UserDependencyConfig}
 */
module.exports = {
  dependency: {
    platforms: {
      android: {
        cxxModuleCMakeListsModuleName: 'cribra-react-native',
        cxxModuleCMakeListsPath: 'CMakeLists.txt',
        cxxModuleHeaderName: 'CribraReactNative',
      },
    },
  },
};
