// Imported first by main.tsx: Cesium resolves its web workers and assets
// relative to this URL. They are copied to public/cesium/ at build time, so
// nothing is fetched from a CDN.
declare global {
  interface Window {
    CESIUM_BASE_URL?: string;
  }
}

window.CESIUM_BASE_URL = `${import.meta.env.BASE_URL}cesium/`;

export {};
