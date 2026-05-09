export interface IconSelectionState {
  selectedIds: string[];
  anchorId: string | null;
}

export interface IconSelectionModifiers {
  ctrlKey?: boolean;
  metaKey?: boolean;
  shiftKey?: boolean;
}

export function selectIcon(
  current: IconSelectionState,
  orderedIds: string[],
  targetId: string,
  modifiers: IconSelectionModifiers,
): IconSelectionState {
  if (!orderedIds.includes(targetId)) {
    return current;
  }

  const shouldToggle = modifiers.ctrlKey === true || modifiers.metaKey === true;
  const shouldRangeSelect = modifiers.shiftKey === true;

  if (shouldRangeSelect) {
    const anchorId = current.anchorId && orderedIds.includes(current.anchorId)
      ? current.anchorId
      : targetId;
    const rangeIds = idsBetween(orderedIds, anchorId, targetId);
    const selectedIds = shouldToggle
      ? orderedSelectedIds(orderedIds, new Set([...current.selectedIds, ...rangeIds]))
      : rangeIds;

    return {
      selectedIds,
      anchorId,
    };
  }

  if (shouldToggle) {
    const selectedIds = new Set(current.selectedIds);
    if (selectedIds.has(targetId)) {
      selectedIds.delete(targetId);
    } else {
      selectedIds.add(targetId);
    }

    return {
      selectedIds: orderedSelectedIds(orderedIds, selectedIds),
      anchorId: targetId,
    };
  }

  return {
    selectedIds: [targetId],
    anchorId: targetId,
  };
}

export function selectIconForContextMenu(
  current: IconSelectionState,
  orderedIds: string[],
  targetId: string,
): IconSelectionState {
  if (current.selectedIds.includes(targetId)) {
    return current;
  }

  return selectIcon(current, orderedIds, targetId, {});
}

export function pruneSelection(
  current: IconSelectionState,
  orderedIds: string[],
): IconSelectionState {
  const orderedIdSet = new Set(orderedIds);
  const selectedIds = current.selectedIds.filter((id) => orderedIdSet.has(id));
  const anchorId =
    current.anchorId && orderedIdSet.has(current.anchorId) ? current.anchorId : null;

  return {
    selectedIds,
    anchorId,
  };
}

function idsBetween(orderedIds: string[], firstId: string, secondId: string) {
  const firstIndex = orderedIds.indexOf(firstId);
  const secondIndex = orderedIds.indexOf(secondId);

  if (firstIndex === -1 || secondIndex === -1) {
    return [secondId];
  }

  const start = Math.min(firstIndex, secondIndex);
  const end = Math.max(firstIndex, secondIndex);
  return orderedIds.slice(start, end + 1);
}

function orderedSelectedIds(orderedIds: string[], selectedIds: Set<string>) {
  return orderedIds.filter((id) => selectedIds.has(id));
}
