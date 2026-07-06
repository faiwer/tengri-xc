// Greifenburg
export const DEFAULT_CENTER = { lat: 46.751, lng: 13.1786 };

export const DEFAULT_ZOOM = 10;

export const PADDING_PX = 32;

/**
 * Pixels by which the MapLibre canvas overhangs the visible container on every
 * side. The container clips the overflow with `overflow: hidden`, so the halo
 * is invisible — but MapLibre still fetches tiles for it, making pans into
 * freshly-revealed area paint instantly instead of showing a blank tile while
 * the request flies. Mirrored in `MapView.module.scss` (`.mapBuffer` `inset`).
 *
 * Any `fitBounds` / `setPadding` call has to add this to its requested padding
 * so the *visible* viewport still respects the requested inset, not the larger
 * canvas viewport.
 */
export const PREFETCH_BUFFER_PX = 256;
