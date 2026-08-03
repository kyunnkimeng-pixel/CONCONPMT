import type { TextOverlaySettings } from "@/features/editor/types";

export interface IconRequestLifecycle {
  iconId: string;
  epoch: number;
  mounted: boolean;
}

export interface IconRequestToken {
  iconId: string;
  epoch: number;
}

export interface TextOverlayDraftState {
  enabled: boolean;
  text: string;
  fontPath: string;
  fontSize: number;
  xPercent: number;
  yPercent: number;
  color: string;
  strokeColor: string;
  strokeWidth: number;
}

export function isTextOverlayDraftDirty(
  draft: TextOverlayDraftState,
  saved: TextOverlaySettings,
) {
  return (
    draft.enabled !== saved.enabled ||
    draft.text !== saved.text ||
    draft.fontPath !== (saved.fontPath ?? "") ||
    draft.fontSize !== saved.fontSize ||
    draft.xPercent !== Math.round(saved.x * 100) ||
    draft.yPercent !== Math.round(saved.y * 100) ||
    draft.color !== saved.color ||
    draft.strokeColor !== saved.strokeColor ||
    draft.strokeWidth !== saved.strokeWidth
  );
}

export function hasUnsavedEditorChanges(
  mainDraftDirty: boolean,
  advancedDraftDirty: boolean,
) {
  return mainDraftDirty || advancedDraftDirty;
}

function hasCommandErrorCode(error: unknown, code: string) {
  return (
    typeof error === "object" &&
    error !== null &&
    "code" in error &&
    (error as { code?: unknown }).code === code
  );
}

export function isRevisionConflict(error: unknown) {
  return hasCommandErrorCode(error, "conflict");
}

export function isAiSourceRepairRequired(error: unknown) {
  return hasCommandErrorCode(error, "ai_source_repair_required");
}

export function isStaleMeasurement(error: unknown) {
  return hasCommandErrorCode(error, "stale_measurement");
}

export function createIconRequestLifecycle(
  iconId: string,
): IconRequestLifecycle {
  return {
    iconId,
    epoch: 0,
    mounted: true,
  };
}

export function activateIconRequestLifecycle(
  lifecycle: IconRequestLifecycle,
  iconId: string,
) {
  if (lifecycle.iconId !== iconId) {
    lifecycle.iconId = iconId;
    lifecycle.epoch += 1;
  }
  lifecycle.mounted = true;
}

export function invalidateIconRequestLifecycle(
  lifecycle: IconRequestLifecycle,
) {
  lifecycle.epoch += 1;
  lifecycle.mounted = false;
}

export function captureIconRequest(
  lifecycle: IconRequestLifecycle,
): IconRequestToken {
  return {
    iconId: lifecycle.iconId,
    epoch: lifecycle.epoch,
  };
}

export function isIconRequestCurrent(
  lifecycle: IconRequestLifecycle,
  token: IconRequestToken,
) {
  return (
    lifecycle.mounted &&
    lifecycle.iconId === token.iconId &&
    lifecycle.epoch === token.epoch
  );
}

export function isEditorStateResponseCurrent(
  activeIconId: string,
  responseIconId: string,
) {
  return activeIconId === responseIconId;
}

export function effectPreviewRequestKey(input: {
  iconId: string;
  iconUpdatedAt: string;
  effectRevision: number;
  draftSignature: string;
}) {
  return JSON.stringify([
    input.iconId,
    input.iconUpdatedAt,
    input.effectRevision,
    input.draftSignature,
  ]);
}
