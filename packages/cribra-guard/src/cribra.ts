/**
 * The extension consumes the repository TypeScript wrapper directly so the
 * qualification build is tied to the reviewed source, not an npm publication.
 *
 * The build resolves `./wasm/cribra.js` inside the wrapper to a separately
 * built production wasm-bindgen glue module. See scripts/build.mjs.
 */
export * from "../../cribra/src/index.js";
