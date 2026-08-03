import { normalizeIconSummary } from "@/features/icons/api";
import type { SheetGridAnalysis, SheetGridSettings } from "@/features/sheets/types";
import { filePathToAssetUrl } from "@/lib/asset-url";
import {
  fileToImportPayload,
  filesToImportPayloads,
} from "@/lib/import-file";
import { invokeCommand } from "@/lib/tauri";
import type {
  AiGeneratedIconsCommitResult,
  AiGridInputDragResult,
  AiGridLayout,
  AiGridReviewCommitResult,
  AiGridWorkspace,
  FinalizeGeneratedIconInput,
  ReviewedAiGridDecision,
} from "@/features/ai-grid/types";

export const MAX_AI_REFERENCE_EXTERNAL_BYTES = 16 * 1024 * 1024;

function normalizeWorkspace(workspace: AiGridWorkspace): AiGridWorkspace {
  const normalizeArtifact = (artifact: AiGridWorkspace["inputArtifact"]) =>
    artifact
      ? {
          ...artifact,
          previewUrl:
            filePathToAssetUrl(artifact.filePath, artifact.sha256) ?? "",
        }
      : null;
  return {
    ...workspace,
    inputArtifact: normalizeArtifact(workspace.inputArtifact),
    outputArtifact: normalizeArtifact(workspace.outputArtifact),
  };
}

export function prepareAiGridEditWorkspace(
  collectionId: string,
  selectedIconIds: string[],
  layout: AiGridLayout | null = null,
) {
  return invokeCommand<AiGridWorkspace>("prepare_ai_grid_edit_workspace", {
    collectionId,
    payload: {
      selectedIconIds,
      layout,
      canvasSize: layout ? null : 1024,
      retryOfRequestId: null,
    },
  }).then(normalizeWorkspace);
}

export async function prepareAiGenerationWorkspace(
  collectionId: string,
  targetNames: string[],
  payloadInputSignature: string,
  layout: AiGridLayout | null = null,
  referenceIconIds: string[] = [],
  referenceFiles: File[] = [],
) {
  const totalReferenceBytes = referenceFiles.reduce(
    (total, file) => total + file.size,
    0,
  );
  if (totalReferenceBytes > MAX_AI_REFERENCE_EXTERNAL_BYTES) {
    throw new Error("외부 참고 이미지는 합계 16MB까지 사용할 수 있습니다.");
  }
  const referenceFilePayloads = await filesToImportPayloads(referenceFiles);
  return invokeCommand<AiGridWorkspace>("prepare_ai_generation_workspace", {
    collectionId,
    payload: {
      targetNames,
      layout,
      canvasSize: layout ? null : 1024,
      payloadInputSignature,
      referenceIconIds,
      referenceFiles: referenceFilePayloads,
      retryOfRequestId: null,
    },
  }).then(normalizeWorkspace);
}

export function getAiGridWorkspace(requestId: string) {
  return invokeCommand<AiGridWorkspace>("get_ai_grid_workspace", {
    requestId,
  }).then(normalizeWorkspace);
}

export function getLatestAiGridWorkspace(collectionId: string) {
  return invokeCommand<AiGridWorkspace | null>("get_latest_ai_grid_workspace", {
    collectionId,
  }).then((workspace) => (workspace ? normalizeWorkspace(workspace) : null));
}

export function markAiGridWorkspaceAwaitingResult(requestId: string) {
  return invokeCommand<AiGridWorkspace>(
    "mark_ai_grid_workspace_awaiting_result",
    { requestId },
  ).then(normalizeWorkspace);
}

export async function attachAiGridOutput(
  requestId: string,
  file: File,
  allowOpaqueBackground = false,
) {
  return invokeCommand<AiGridWorkspace>("attach_ai_grid_output", {
    requestId,
    file: await fileToImportPayload(file),
    manifestJson: null,
    allowOpaqueBackground,
  }).then(normalizeWorkspace);
}

export function analyzeAiGridOutput(
  requestId: string,
  settings: SheetGridSettings,
) {
  return invokeCommand<SheetGridAnalysis>("analyze_ai_grid_output", {
    requestId,
    settings,
  });
}

export function commitAiGridReview(
  requestId: string,
  decisions: ReviewedAiGridDecision[],
) {
  return invokeCommand<AiGridReviewCommitResult>("commit_ai_grid_review", {
    requestId,
    decisions,
  }).then((result) => ({
    ...result,
    workspace: normalizeWorkspace(result.workspace),
  }));
}

export function commitAiGeneratedIcons(
  collectionId: string,
  requestId: string,
  finalizedItems: FinalizeGeneratedIconInput[],
) {
  return invokeCommand<AiGeneratedIconsCommitResult>(
    "commit_ai_generated_icons",
    { collectionId, requestId, finalizedItems },
  ).then((result) => ({
    ...result,
    commit: {
      ...result.commit,
      createdIcons: result.commit.createdIcons.map(normalizeIconSummary),
    },
    workspace: normalizeWorkspace(result.workspace),
  }));
}

export function cancelAiGridWorkspace(requestId: string) {
  return invokeCommand<AiGridWorkspace>("cancel_ai_grid_workspace", {
    requestId,
  }).then(normalizeWorkspace);
}

export function revealAiGridInput(requestId: string) {
  return invokeCommand<void>("reveal_ai_grid_input", { requestId });
}

export function startAiGridInputDrag(requestId: string) {
  return invokeCommand<AiGridInputDragResult>("start_ai_grid_input_drag", {
    requestId,
  });
}