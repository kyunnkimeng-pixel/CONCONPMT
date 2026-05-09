import { normalizeIconSummary } from "@/features/icons/api";
import type { IconSummary } from "@/features/collections/types";
import type {
  ApplyIconCropInput,
  IconEditorState,
  SourceFileSummary,
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

function normalizeIconEditorState(state: IconEditorState): IconEditorState {
  return {
    ...state,
    icon: normalizeIconSummary(state.icon),
    source: normalizeSourceFile(state.source),
  };
}

function normalizeSourceFile(source: SourceFileSummary): SourceFileSummary {
  return {
    ...source,
    originalImageUrl: filePathToAssetUrl(source.originalImageUrl) ?? "",
  };
}
