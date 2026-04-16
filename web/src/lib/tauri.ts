// Tauri detection utilities for NaraeClaw Desktop.

declare global {
  interface Window {
    __TAURI__?: unknown;
    __NARAECLAW_GATEWAY__?: string;
    __ZEROCLAW_GATEWAY__?: string;
  }
}

/** Returns true when running inside a Tauri WebView. */
export const isTauri = (): boolean => '__TAURI__' in window;

/** Gateway base URL when running inside Tauri (defaults to localhost). */
export const tauriGatewayUrl = (): string =>
  window.__NARAECLAW_GATEWAY__ ?? window.__ZEROCLAW_GATEWAY__ ?? 'http://127.0.0.1:42617';
