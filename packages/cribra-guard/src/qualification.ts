/**
 * Qualification-only extension page.
 *
 * This page is the external message peer for the service worker so automated
 * tests exercise the real `chrome.runtime` path instead of importing worker
 * internals. It is not a Guard UI and is never registered in the manifest.
 */

interface QualificationBridge {
  send(message: unknown): Promise<unknown>;
  sendAndForget(message: unknown): void;
  probe(): {
    readonly origin: string;
    readonly hasStoragePermission: boolean;
    readonly hasStorageApi: boolean;
    readonly hasTabsPermission: boolean;
  };
}

declare global {
  interface Window {
    cribraQualification: QualificationBridge;
  }
}

export {};

window.cribraQualification = {
  send(message) {
    return chrome.runtime.sendMessage(message);
  },
  sendAndForget(message) {
    void chrome.runtime.sendMessage(message).catch(() => {
      // The page is expected to disappear mid-request.
    });
  },
  probe() {
    const manifest = chrome.runtime.getManifest();
    return {
      origin: location.origin,
      hasStoragePermission: manifest.permissions?.includes("storage") ?? false,
      hasStorageApi: typeof chrome.storage !== "undefined",
      hasTabsPermission: manifest.permissions?.includes("tabs") ?? false,
    };
  },
};
