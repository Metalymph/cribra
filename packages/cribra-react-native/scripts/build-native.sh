#!/usr/bin/env bash
# Builds the Cribra C ABI static libraries used by the private React Native
# package. Outputs stay under build/ and android/src/main/cpp/prebuilt/ and are
# intentionally ignored by Git.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PACKAGE_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
REPO_ROOT="$(cd "${PACKAGE_DIR}/../.." && pwd)"
BUILD_DIR="${PACKAGE_DIR}/build"
IOS_XCFRAMEWORK="${PACKAGE_DIR}/ios/CribraCAPI.xcframework"
ANDROID_PREBUILT="${PACKAGE_DIR}/android/src/main/cpp/prebuilt"
TARGET_DIR="${REPO_ROOT}/target"
NDK_VERSION="${CRIBRA_NDK_VERSION:-27.1.12297006}"

platform="${1:-all}"

build_ios() {
  echo "==> Building Cribra C ABI for iOS"
  cargo build \
    --manifest-path "${REPO_ROOT}/Cargo.toml" \
    --package cribra-capi \
    --release \
    --target aarch64-apple-ios
  cargo build \
    --manifest-path "${REPO_ROOT}/Cargo.toml" \
    --package cribra-capi \
    --release \
    --target aarch64-apple-ios-sim
  cargo build \
    --manifest-path "${REPO_ROOT}/Cargo.toml" \
    --package cribra-capi \
    --release \
    --target x86_64-apple-ios

  mkdir -p "${BUILD_DIR}/ios/device" "${BUILD_DIR}/ios/simulator"
  cp "${TARGET_DIR}/aarch64-apple-ios/release/libcribra_capi.a" \
    "${BUILD_DIR}/ios/device/libcribra_capi.a"
  lipo -create \
    "${TARGET_DIR}/aarch64-apple-ios-sim/release/libcribra_capi.a" \
    "${TARGET_DIR}/x86_64-apple-ios/release/libcribra_capi.a" \
    -output "${BUILD_DIR}/ios/simulator/libcribra_capi.a"

  rm -rf "${IOS_XCFRAMEWORK}"
  xcodebuild -create-xcframework \
    -library "${BUILD_DIR}/ios/device/libcribra_capi.a" \
    -library "${BUILD_DIR}/ios/simulator/libcribra_capi.a" \
    -output "${IOS_XCFRAMEWORK}"
  echo "==> iOS artifact: ${IOS_XCFRAMEWORK}"
}

build_android() {
  echo "==> Building Cribra C ABI for Android (NDK ${NDK_VERSION})"
  export ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-${HOME}/Library/Android/sdk/ndk/${NDK_VERSION}}"
  if [[ ! -d "${ANDROID_NDK_HOME}" ]]; then
    echo "Android NDK not found at ${ANDROID_NDK_HOME}" >&2
    exit 1
  fi

  local abi triple
  for abi in armeabi-v7a arm64-v8a x86 x86_64; do
    case "${abi}" in
      armeabi-v7a) triple="armv7-linux-androideabi" ;;
      arm64-v8a) triple="aarch64-linux-android" ;;
      x86) triple="i686-linux-android" ;;
      x86_64) triple="x86_64-linux-android" ;;
    esac

    cargo ndk \
      --manifest-path "${REPO_ROOT}/Cargo.toml" \
      --platform 24 \
      --target "${abi}" \
      build --package cribra-capi --release

    mkdir -p "${ANDROID_PREBUILT}/${abi}"
    cp "${TARGET_DIR}/${triple}/release/libcribra_capi.a" \
      "${ANDROID_PREBUILT}/${abi}/libcribra_capi.a"
    echo "==> Android ${abi}: ${ANDROID_PREBUILT}/${abi}/libcribra_capi.a"
  done
}

case "${platform}" in
  ios) build_ios ;;
  android) build_android ;;
  all)
    build_ios
    build_android
    ;;
  *)
    echo "Usage: $0 [ios|android|all]" >&2
    exit 2
    ;;
esac
