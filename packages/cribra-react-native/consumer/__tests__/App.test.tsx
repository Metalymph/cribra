/**
 * @format
 */

import React from 'react';
import ReactTestRenderer from 'react-test-renderer';

// Jest has no native TurboModule. Replace the facade with a small, coherent
// fake so the screen can render; the real harness is exercised on a simulator
// and emulator, not in unit tests.
jest.mock('@metalymph/cribra-react-native', () => {
  class CribraError extends Error {
    code: string;
    detail?: string;

    constructor(code: string, detail?: string) {
      super(code);
      this.name = 'CribraError';
      this.code = code;
      if (detail !== undefined) {
        this.detail = detail;
      }
    }
  }

  return {
    CribraError,
    getAbiVersion: () => ({ major: 0, minor: 3, patch: 0 }),
    getLiveScannerCount: () => 0,
    createScanner: () =>
      Promise.reject(new CribraError('scanner_create_failed')),
  };
});

import App from '../App';

test('renders correctly', async () => {
  const log = jest.spyOn(console, 'log').mockImplementation(() => {});
  let tree!: ReactTestRenderer.ReactTestRenderer;

  await ReactTestRenderer.act(async () => {
    tree = ReactTestRenderer.create(<App />);
  });

  await ReactTestRenderer.act(async () => {
    tree.unmount();
  });

  log.mockRestore();
});
