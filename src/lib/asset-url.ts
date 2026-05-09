import { convertFileSrc } from "@tauri-apps/api/core";

export function filePathToAssetUrl(path: string | null, cacheKey?: string | null) {
  if (!path) {
    return null;
  }

  if (
    path.startsWith("asset:") ||
    path.startsWith("http://asset.localhost") ||
    path.startsWith("https://asset.localhost") ||
    path.startsWith("data:")
  ) {
    return appendCacheKey(path, cacheKey);
  }

  return appendCacheKey(convertFileSrc(path), cacheKey);
}

function appendCacheKey(url: string, cacheKey?: string | null) {
  if (!cacheKey || url.startsWith("data:")) {
    return url;
  }

  return `${url}${url.includes("?") ? "&" : "?"}v=${encodeURIComponent(cacheKey)}`;
}
