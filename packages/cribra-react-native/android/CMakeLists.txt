cmake_minimum_required(VERSION 3.13)
project(cribra_react_native)

set(CRIBRA_CAPI_PREBUILT
    "${CMAKE_CURRENT_SOURCE_DIR}/src/main/cpp/prebuilt/${ANDROID_ABI}/libcribra_capi.a")

if(NOT EXISTS "${CRIBRA_CAPI_PREBUILT}")
  message(FATAL_ERROR
    "Cribra React Native native library is missing for ABI '${ANDROID_ABI}'. "
    "Run packages/cribra-react-native/scripts/build-native.sh before building Android.")
endif()

add_library(cribra_capi STATIC IMPORTED)
set_target_properties(cribra_capi PROPERTIES
  IMPORTED_LOCATION "${CRIBRA_CAPI_PREBUILT}")

add_library(cribra-react-native STATIC
  ../cpp/CribraBridge.cpp
  ../cpp/CribraReactNative.cpp)

set_target_properties(cribra-react-native PROPERTIES
  CXX_STANDARD 20
  CXX_STANDARD_REQUIRED ON
  CXX_EXTENSIONS OFF)

target_include_directories(cribra-react-native PUBLIC
  ../cpp
  ../third_party/cribra/include)

target_link_libraries(cribra-react-native
  cribra_capi
  jsi
  reactnative
  react_codegen_CribraReactNativeSpec
  log
  dl
  m)
