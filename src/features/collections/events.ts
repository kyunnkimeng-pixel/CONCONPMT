const COLLECTION_LIST_CHANGED_EVENT = "pmtconcon:collection-list-changed";

export function notifyCollectionListChanged() {
  if (typeof window !== "undefined") {
    window.dispatchEvent(new Event(COLLECTION_LIST_CHANGED_EVENT));
  }
}

export function subscribeCollectionListChanged(listener: () => void) {
  if (typeof window === "undefined") {
    return () => undefined;
  }

  window.addEventListener(COLLECTION_LIST_CHANGED_EVENT, listener);
  return () => window.removeEventListener(COLLECTION_LIST_CHANGED_EVENT, listener);
}
