export function resolveDndAccessibilityContainer(
  suppressBackgroundLiveRegions: boolean,
  detachedContainer: Element | null,
) {
  return suppressBackgroundLiveRegions
    ? (detachedContainer ?? undefined)
    : undefined;
}
