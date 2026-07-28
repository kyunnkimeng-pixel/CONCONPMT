import type { IconSummary } from "@/features/collections/types";

export function upsertIconSummary(
  icons: readonly IconSummary[],
  updatedIcon: IconSummary,
) {
  const hasExistingIcon = icons.some((icon) => icon.id === updatedIcon.id);
  const nextIcons = hasExistingIcon
    ? icons.map((icon) => (icon.id === updatedIcon.id ? updatedIcon : icon))
    : [...icons, updatedIcon];

  return nextIcons.sort(
    (left, right) =>
      left.orderIndex - right.orderIndex || left.id.localeCompare(right.id),
  );
}
