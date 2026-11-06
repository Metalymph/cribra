/* Vite consumer entry. Imports the installed package by name. */

import * as api from "cribra-typescript";
import { runAppSuite } from "./app-suite.mjs";

const result = document.getElementById("result");

runAppSuite(api, { expectedVersion: import.meta.env?.VITE_CRIBRA_VERSION })
  .then(() => {
    result.textContent = "PASS";
  })
  .catch((error) => {
    const message = error && error.message ? error.message : String(error);
    const code = error && error.code ? ` [${error.code}]` : "";
    result.textContent = `FAIL: ${message}${code}`;
  });
