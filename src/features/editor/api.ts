import { normalizeIconSummary } from "@/features/icons/api";
import {
  aiCandidateFileFormatError,
  aiCandidateFileSizeError,
} from "@/features/editor/ai-review-model";
import type { IconSummary } from "@/features/collections/types";
import type {
  ActivateAiCandidateInput,
  AiCandidateUsageSummary,
  AiManualServiceSurface,
  AiImageEditInput,
  AiOfficialResource,
  AiProvider,
  AiProviderSessionStatus,
  AiWebHandoffDeleteResult,
  AiWebHandoffDragResult,
  AiWebHandoffResultInspection,
  AiWebHandoffServiceSurface,
  AiWebHandoffSession,
  AiNormalizationPreview,
  AiReviewState,
  AiSourceMutationResult,
  ApplyIconCropInput,
  CreateAiIconRootInput,
  CreateAiIconRootResult,
  EffectiveVisualSource,
  IconEffectPreview,
  IconEditorState,
  MotionPreviewDto,
  PreviewIconEffectsInput,
  PreviewIconMotionInput,
  RestoreAiVersionInput,
  PreviewAiCandidateNormalizationInput,
  SourceFileSummary,
  UpdateIconEffectsInput,
  UpdateIconMotionInput,
  UpdateIconTextOverlayInput,
} from "@/features/editor/types";
import { filePathToAssetUrl } from "@/lib/asset-url";
import { fileToImportPayload } from "@/lib/import-file";
import { invokeCommand } from "@/lib/tauri";

export function getIconEditorState(collectionId: string, iconId: string) {
  return invokeCommand<IconEditorState>("get_icon_editor_state", {
    collectionId,
    iconId,
  }).then(normalizeIconEditorState);
}

export function getAiReviewState(collectionId: string, iconId: string) {
  return invokeCommand<AiReviewState>("get_ai_review_state", {
    collectionId,
    iconId,
  }).then(normalizeAiReviewState);
}

export function getAiProviderSessionStatus() {
  return invokeCommand<AiProviderSessionStatus>(
    "get_ai_provider_session_status",
    {},
  );
}

export function setAiSessionCredential(
  provider: AiProvider,
  credential: string,
) {
  return invokeCommand<AiProviderSessionStatus>("set_ai_session_credential", {
    payload: { provider, credential },
  });
}

export function clearAiSessionCredential(provider: AiProvider) {
  return invokeCommand<AiProviderSessionStatus>("clear_ai_session_credential", {
    provider,
  });
}

export function executeAiImageEdit(
  collectionId: string,
  payload: AiImageEditInput,
) {
  return invokeCommand<AiReviewState>("execute_ai_image_edit", {
    collectionId,
    payload,
  }).then(normalizeAiReviewState);
}

export function openAiOfficialResource(resource: AiOfficialResource) {
  return invokeCommand<void>("open_ai_official_resource", { resource });
}

export function prepareAiWebHandoff(
  collectionId: string,
  iconId: string,
  serviceSurface: AiWebHandoffServiceSurface,
  userPrompt: string,
) {
  return invokeCommand<AiWebHandoffSession>("prepare_ai_web_handoff", {
    collectionId,
    payload: { iconId, serviceSurface, userPrompt },
  }).then(normalizeAiWebHandoffSession);
}

export function getAiWebHandoff(requestId: string) {
  return invokeCommand<AiWebHandoffSession>("get_ai_web_handoff", {
    requestId,
  }).then(normalizeAiWebHandoffSession);
}

export function getLatestAiWebHandoffForIcon(
  collectionId: string,
  iconId: string,
) {
  return invokeCommand<AiWebHandoffSession | null>(
    "get_latest_ai_web_handoff_for_icon",
    { collectionId, iconId },
  ).then((session) =>
    session ? normalizeAiWebHandoffSession(session) : null,
  );
}

export function revealAiWebHandoffUpload(requestId: string) {
  return invokeCommand<void>("reveal_ai_web_handoff_upload", { requestId });
}

export function startAiWebHandoffDrag(requestId: string) {
  return invokeCommand<AiWebHandoffDragResult>("start_ai_web_handoff_drag", {
    requestId,
  });
}

export async function validateAiWebHandoffResult(
  requestId: string,
  file: File,
) {
  return invokeCommand<AiWebHandoffResultInspection>(
    "validate_ai_web_handoff_result",
    { requestId, file: await fileToImportPayload(file) },
  ).then(normalizeAiWebHandoffResultInspection);
}

export async function commitAiWebHandoffResult(
  requestId: string,
  file: File,
  expectedValidationSignature: string,
) {
  return invokeCommand<AiWebHandoffResultInspection>(
    "commit_ai_web_handoff_result",
    {
      requestId,
      file: await fileToImportPayload(file),
      expectedValidationSignature,
    },
  ).then(normalizeAiWebHandoffResultInspection);
}

export async function inspectAndCommitAiWebHandoffResult(
  requestId: string,
  file: File,
) {
  const payload = await fileToImportPayload(file);
  const validation = normalizeAiWebHandoffResultInspection(
    await invokeCommand<AiWebHandoffResultInspection>(
      "validate_ai_web_handoff_result",
      { requestId, file: payload },
    ),
  );
  if (!validation.accepted || !validation.validationSignature) {
    return validation;
  }
  return invokeCommand<AiWebHandoffResultInspection>(
    "commit_ai_web_handoff_result",
    {
      requestId,
      file: payload,
      expectedValidationSignature: validation.validationSignature,
    },
  ).then(normalizeAiWebHandoffResultInspection);
}

export function extendAiWebHandoffRetention(requestId: string) {
  return invokeCommand<AiWebHandoffSession>("extend_ai_web_handoff_retention", {
    requestId,
  }).then(normalizeAiWebHandoffSession);
}

export function deleteAiWebHandoffPayload(requestId: string) {
  return invokeCommand<AiWebHandoffDeleteResult>(
    "delete_ai_web_handoff_payload",
    { requestId },
  );
}
export async function importLocalAiCandidate(
  collectionId: string,
  iconId: string,
  serviceSurface: AiManualServiceSurface,
  file: File,
) {
  const formatError = aiCandidateFileFormatError(file);
  if (formatError) {
    throw new Error(formatError);
  }

  const sizeError = aiCandidateFileSizeError(file);
  if (sizeError) {
    throw new Error(sizeError);
  }

  return invokeCommand<AiReviewState>("import_local_ai_candidate", {
    collectionId,
    payload: {
      iconId,
      serviceSurface,
      file: await fileToImportPayload(file),
    },
  }).then(normalizeAiReviewState);
}

export function activateAiCandidate(
  collectionId: string,
  payload: ActivateAiCandidateInput,
) {
  return invokeCommand<AiSourceMutationResult>("activate_ai_candidate", {
    collectionId,
    payload,
  }).then(normalizeAiSourceMutationResult);
}
export function previewAiCandidateNormalization(
  collectionId: string,
  payload: PreviewAiCandidateNormalizationInput,
) {
  return invokeCommand<AiNormalizationPreview>(
    "preview_ai_candidate_normalization",
    {
      collectionId,
      payload,
    },
  ).then(normalizeAiNormalizationPreview);
}

export function createAiIconRoot(
  collectionId: string,
  payload: CreateAiIconRootInput,
) {
  return invokeCommand<CreateAiIconRootResult>("create_ai_icon_root", {
    collectionId,
    payload,
  }).then((result) => ({
    createdIcon: normalizeIconSummary(result.createdIcon),
    sourceReviewState: normalizeAiReviewState(result.sourceReviewState),
    createdIconUsage: normalizeAiCandidateUsage(result.createdIconUsage),
  }));
}

export function restoreAiVersion(
  collectionId: string,
  payload: RestoreAiVersionInput,
) {
  return invokeCommand<AiSourceMutationResult>("restore_ai_version", {
    collectionId,
    payload,
  }).then(normalizeAiSourceMutationResult);
}

export function repairAiToOriginal(collectionId: string, iconId: string) {
  return invokeCommand<AiReviewState>("repair_ai_to_original", {
    collectionId,
    payload: {
      iconId,
    },
  }).then(normalizeAiReviewState);
}

export function applyIconCrop(
  collectionId: string,
  payload: ApplyIconCropInput,
) {
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
  const visualSource = normalizeEffectiveVisualSource(state.visualSource);
  return {
    ...state,
    icon: normalizeIconSummary(state.icon),
    source: normalizeSourceFile(state.source),
    visualSource,
  };
}

function normalizeAiReviewState(state: AiReviewState): AiReviewState {
  return {
    ...state,
    visualSource: normalizeEffectiveVisualSource(state.visualSource),
    candidates: state.candidates.map((candidate) => ({
      ...candidate,
      source: normalizeSourceFile(candidate.source),
      createdIconUsage: normalizeAiCandidateUsage(candidate.createdIconUsage),
    })),
    versions: state.versions.map((version) => ({
      ...version,
      source: normalizeSourceFile(version.source),
    })),
  };
}

function normalizeAiWebHandoffSession(
  session: AiWebHandoffSession,
): AiWebHandoffSession {
  return {
    ...session,
    uploadPreviewPath:
      filePathToAssetUrl(session.uploadPreviewPath, session.requestId) ?? "",
  };
}

function normalizeAiWebHandoffResultInspection(
  inspection: AiWebHandoffResultInspection,
): AiWebHandoffResultInspection {
  return {
    ...inspection,
    reviewState: inspection.reviewState
      ? normalizeAiReviewState(inspection.reviewState)
      : null,
  };
}

function normalizeAiSourceMutationResult(
  result: AiSourceMutationResult,
): AiSourceMutationResult {
  return {
    reviewState: normalizeAiReviewState(result.reviewState),
    editorState: normalizeIconEditorState(result.editorState),
  };
}

function normalizeAiCandidateUsage(
  usage: AiCandidateUsageSummary,
): AiCandidateUsageSummary {
  return {
    ...usage,
    latestCreatedIcon: usage.latestCreatedIcon
      ? normalizeIconSummary(usage.latestCreatedIcon)
      : null,
  };
}

function normalizeEffectiveVisualSource(
  visualSource: EffectiveVisualSource,
): EffectiveVisualSource {
  return {
    ...visualSource,
    originalSource: normalizeSourceFile(visualSource.originalSource),
    effectiveRenderSource: normalizeSourceFile(
      visualSource.effectiveRenderSource,
    ),
  };
}

function normalizeAiNormalizationPreview(
  preview: AiNormalizationPreview,
): AiNormalizationPreview {
  return {
    ...preview,
    rawSource: normalizeSourceFile(preview.rawSource),
    normalizedPreviewPath:
      filePathToAssetUrl(
        preview.normalizedPreviewPath,
        preview.previewSignature,
      ) ?? "",
    finalPreviewPath:
      filePathToAssetUrl(preview.finalPreviewPath, preview.previewSignature) ??
      "",
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
    posterPath:
      filePathToAssetUrl(preview.posterPath, preview.generatedAt) ?? "",
  };
}

function normalizeSourceFile(source: SourceFileSummary): SourceFileSummary {
  return {
    ...source,
    originalImageUrl: filePathToAssetUrl(source.originalImageUrl) ?? "",
  };
}
