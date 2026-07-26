import { normalizeIconSummary } from "@/features/icons/api";
import type { IconSummary } from "@/features/collections/types";
import type {
  ApplyIconCropInput,
  IconEffectPreview,
  IconEditorState,
  MotionPreviewDto,
  PreviewIconEffectsInput,
  PreviewIconMotionInput,
  SourceFileSummary,
  UpdateIconEffectsInput,
  UpdateIconMotionInput,
  UpdateIconTextOverlayInput,
} from "@/features/editor/types";
import { filePathToAssetUrl } from "@/lib/asset-url";
import { invokeCommand } from "@/lib/tauri";

export function getIconEditorState(collectionId: string, iconId: string) {
  return invokeCommand<IconEditorState>("get_icon_editor_state", {
    collectionId,
    iconId,
  }).then(normalizeIconEditorState);
}

export function applyIconCrop(collectionId: string, payload: ApplyIconCropInput) {
  return invokeCommand<IconSummary>("apply_icon_crop", {
    collectionId,
    payload,
  }).then(normalizeIconSummary);
}

export function updateIconTextOverlay(
  collectionId: string,
  payload: UpdateIconTextOverlayInput,
) {
  return invokeCommand<IconEditorState>("update_icon_text_overlay", {
    collectionId,
    payload,
  }).then(normalizeIconEditorState);
}

export function pickTextOverlayFont(initialDirectory: string | null) {
  return invokeCommand<string | null>("pick_text_overlay_font", {
    initialDirectory,
  });
}

export function previewIconEffects(
  collectionId: string,
  payload: PreviewIconEffectsInput,
) {
  return invokeCommand<IconEffectPreview>("preview_icon_effects", {
    collectionId,
    payload,
  }).then(normalizeIconEffectPreview);
}

export function updateIconEffects(
  collectionId: string,
  payload: UpdateIconEffectsInput,
) {
  return invokeCommand<IconEditorState>("update_icon_effects", {
    collectionId,
    payload,
  }).then(normalizeIconEditorState);
}

export function previewIconMotion(
  collectionId: string,
  payload: PreviewIconMotionInput,
) {
  return invokeCommand<MotionPreviewDto>("preview_icon_motion", {
    collectionId,
    payload,
  }).then(normalizeMotionPreview);
}

export function updateIconMotion(
  collectionId: string,
  payload: UpdateIconMotionInput,
) {
  return invokeCommand<IconEditorState>("update_icon_motion", {
    collectionId,
    payload,
  }).then(normalizeIconEditorState);
}

function normalizeIconEditorState(state: IconEditorState): IconEditorState {
  return {
    ...state,
    icon: normalizeIconSummary(state.icon),
    source: normalizeSourceFile(state.source),
  };
}

function normalizeIconEffectPreview(
  preview: IconEffectPreview,
): IconEffectPreview {
  return {
    ...preview,
    previewPath:
      filePathToAssetUrl(preview.previewPath, preview.generatedAt) ?? "",
  };
}

function normalizeMotionPreview(preview: MotionPreviewDto): MotionPreviewDto {
  return {
    ...preview,
    previewPath:
      filePathToAssetUrl(preview.previewPath, preview.generatedAt) ?? "",
    posterPath: filePathToAssetUrl(preview.posterPath, preview.generatedAt) ?? "",
  };
}

function normalizeSourceFile(source: SourceFileSummary): SourceFileSummary {
  return {
    ...source,
    originalImageUrl: filePathToAssetUrl(source.originalImageUrl) ?? "",
  };
}
