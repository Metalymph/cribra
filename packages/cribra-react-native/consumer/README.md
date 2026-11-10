# CribraConsumer

Isolated React Native 0.88.1 New Architecture/Hermes consumer for
[`@metalymph/cribra-react-native`](../README.md). It is intentionally separate
from Amon and from any existing Cribra workspace alias.

`App.tsx` runs the synthetic-fixture parity, RN.1.2 batch, lifecycle, and
deterministic native negative harness. On success it logs and displays:

```text
CRIBRA_RN_SMOKE:PASS:abi=0.3.0:entries=5:findings=3:candidates=1:lifecycle=ok:parity=ok:batch=ok:negative=ok:privacy=ok
```

It logs only aggregate counts and pass/fail data, never source text or
individual findings.

`npm test` runs the Jest facade batch-contract tests and the App render check
with the native TurboModule mocked, because unit tests have no native binary.
It does not replace the simulator/emulator harness.

## Run

```sh
npm install
cd ios && pod install
# or, for Android, create android/local.properties with sdk.dir=...
npx react-native start --port 8081
```

The native Rust static libraries must be built from the parent package first:

```sh
cd ..
bash scripts/build-native.sh ios       # or android
```

See the parent [README](../README.md) and
[QUALIFICATION](../QUALIFICATION.md) for build commands and evidence.
