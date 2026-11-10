// Host-side check for the ABI compatibility policy used by the native bridge.
// It includes only the pure header, so it needs no React Native toolchain.
//
// Build and run:
//   clang++ -std=c++17 -I third_party/cribra/include \
//     tools/abi-check/main.cpp -o /tmp/cribra-abi-check && /tmp/cribra-abi-check

#include "../../cpp/CribraAbi.h"

#include <cassert>
#include <cstdio>

int main() {
  using cribra::rn::AbiCompatible;
  using cribra::rn::AbiVersionText;

  // The vendored header currently describes the 0.3 protocol.
  static_assert(cribra::rn::kExpectedAbiMajor == 0);
  static_assert(cribra::rn::kExpectedAbiMinor == 3);

  // Same protocol surface, including any patch difference.
  assert(AbiCompatible(0, 3, 0));
  assert(AbiCompatible(0, 3, 7));

  // Incompatible protocol surfaces fail closed.
  assert(!AbiCompatible(0, 2, 9));
  assert(!AbiCompatible(0, 4, 0));
  assert(!AbiCompatible(1, 3, 0));
  assert(!AbiCompatible(2, 0, 0));
  assert(!AbiCompatible(0, 0, 0));

  assert(AbiVersionText(0, 3, 0) == "0.3.0");
  assert(AbiVersionText(1, 0, 2) == "1.0.2");

  std::puts("ABI compatibility gate: PASS");
  return 0;
}
