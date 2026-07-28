export type IconRevealAction = "focus_tile" | "open_editor";

export interface IconRevealRequest {
  iconId: string;
  action: IconRevealAction;
  requestId: number;
}

export function shouldHandleIconRevealRequest(
  request: IconRevealRequest | null,
  handledRequestId: number | null,
  orderedIconIds: readonly string[],
): request is IconRevealRequest {
  return (
    request !== null &&
    request.requestId !== handledRequestId &&
    orderedIconIds.includes(request.iconId)
  );
}

export function focusRevealedEditorPanel(root: ParentNode) {
  const focusTarget = root.querySelector<HTMLElement>(
    '[data-testid="editor-panel"] button[aria-label="편집 패널 닫기"]',
  );
  if (!focusTarget) {
    return false;
  }

  focusTarget.focus();
  return true;
}