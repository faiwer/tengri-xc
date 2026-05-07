/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** Base URL of the tengri-xc server, including any path prefix. */
  readonly VITE_SERVER_URL: string;
  /** Google Maps JavaScript API key. */
  readonly VITE_GOOGLE_MAPS_API_KEY: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}

declare module '*.module.scss' {
  const classes: { readonly [key: string]: string };
  export default classes;
}
