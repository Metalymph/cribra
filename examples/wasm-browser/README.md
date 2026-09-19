# Cribra browser/WASM example

This example uses Cribra's production browser WebAssembly artifact directly
from a browser-native ES module.

It demonstrates:

- the authoritative default built-in catalog;
- the opt-in financial built-in catalog;
- typed finding metadata;
- caller-owned source text;
- conservative redaction;
- explicit WASM object cleanup.

The example contains synthetic data only.

## Build the production artifact

From the repository root:

```sh
just wasm-production
```

This produces the browser-native `wasm-bindgen --target web` artifact under
`target/wasm-production/`, with the production WASM binary optimized using
Binaryen `-Oz`.

## Run

Serve the repository root through an HTTP server:

```sh
python3 -m http.server 8000
```

Then open `/examples/wasm-browser/` on port 8000 in a browser.

The HTTP server is used only to serve the static HTML, JavaScript, and WASM
files. Cribra scanning and redaction execute locally in the browser.

## Privacy boundary

The example does not send source text to a Cribra service or remote validation
endpoint.

Cribra retains scan metadata rather than source text. The application owns the
source string and supplies it again when requesting redaction.

Real applications remain responsible for browser lifecycle, storage, network
policy, CSP, Worker orchestration, and other application-level behavior.