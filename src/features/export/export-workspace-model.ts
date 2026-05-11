import type {
  ExportItemStatus,
  ExportPlanItem,
  ExportValidationIssue,
  ExportValidationResult,
} from "@/features/export/types";

export type ExportWorkspaceFilter =
  | "all"
  | "included"
  | "excluded"
  | "completed"
  | "pending"
  | "warnings"
  | "not_upload_ready"
  | "failed"
  | "gif"
  | "oversized";

export interface ExportWorkspaceSummary {
  total: number;
  included: number;
  excluded: number;
  uploadReady: number;
  success: number;
  warnings: number;
  notUploadReady: number;
  failed: number;
  oversized: number;
}

export const EXPORT_WORKSPACE_FILTER_LABELS: Record<ExportWorkspaceFilter, string> = {
  all: "전체",
  included: "포함",
  excluded: "제외",
  completed: "완료",
  pending: "대기중",
  warnings: "경고",
  not_upload_ready: "업로드 불가",
  failed: "내보내기 실패",
  gif: "GIF",
  oversized: "용량 초과",
};

export function issuesForItem(
  result: ExportValidationResult | null,
  item: ExportPlanItem,
) {
  if (!result) {
    return [];
  }

  return [...result.errors, ...result.warnings].filter(
    (issue) => issue.pieceId === item.pieceId || issue.iconId === item.iconId,
  );
}

export function hasOversizedIssue(
  result: ExportValidationResult | null,
  item: ExportPlanItem,
) {
  return issuesForItem(result, item).some((issue) => issue.code === "max_bytes");
}

export function summarizeExportWorkspace(
  result: ExportValidationResult | null,
): ExportWorkspaceSummary {
  const items = result?.items ?? [];
  const warningPieceIds = pieceIdSet(result?.warnings ?? []);

  return items.reduce<ExportWorkspaceSummary>(
    (summary, item) => {
      summary.total += 1;
      if (item.included) {
        summary.included += 1;
      } else {
        summary.excluded += 1;
      }

      if (isUploadReadyStatus(item.status)) {
        summary.uploadReady += 1;
      }
      if (isWrittenStatus(item.status)) {
        summary.success += 1;
      }
      if (isWarningStatus(item.status) || warningPieceIds.has(item.pieceId)) {
        summary.warnings += 1;
      }
      if (isNotUploadReadyStatus(item.status)) {
        summary.notUploadReady += 1;
      }
      if (item.status === "failed_to_render") {
        summary.failed += 1;
      }
      if (hasOversizedIssue(result, item)) {
        summary.oversized += 1;
      }

      return summary;
    },
    {
      total: 0,
      included: 0,
      excluded: 0,
      uploadReady: 0,
      success: 0,
      warnings: 0,
      notUploadReady: 0,
      failed: 0,
      oversized: 0,
    },
  );
}

export function filterExportItems(
  result: ExportValidationResult | null,
  filter: ExportWorkspaceFilter,
) {
  const items = result?.items ?? [];
  const warningPieceIds = pieceIdSet(result?.warnings ?? []);

  return items.filter((item) => {
    switch (filter) {
      case "included":
        return item.included;
      case "excluded":
        return !item.included;
      case "completed":
        return isWrittenStatus(item.status);
      case "pending":
        return item.included && isPendingStatus(item.status);
      case "warnings":
        return isWarningStatus(item.status) || warningPieceIds.has(item.pieceId);
      case "not_upload_ready":
        return isNotUploadReadyStatus(item.status);
      case "failed":
        return item.status === "failed_to_render";
      case "gif":
        return item.isAnimated || item.outputFormat === "gif";
      case "oversized":
        return hasOversizedIssue(result, item);
      case "all":
      default:
        return true;
    }
  });
}

export function problemExportNumbers(result: ExportValidationResult | null) {
  const warningPieceIds = pieceIdSet(result?.warnings ?? []);

  return (result?.items ?? [])
    .filter(
      (item) =>
        item.included &&
        (isWarningStatus(item.status) ||
          warningPieceIds.has(item.pieceId) ||
          isNotUploadReadyStatus(item.status) ||
          item.status === "failed_to_render"),
    )
    .map((item) => formatExportIndex(item));
}

export function formatExportIndex(item: ExportPlanItem) {
  if (!item.included || item.exportIndex <= 0) {
    return "-";
  }

  return item.exportIndex.toString().padStart(3, "0");
}

export function statusLabel(status: ExportItemStatus) {
  switch (status) {
    case "excluded":
      return "제외됨";
    case "preflight_ok":
      return "대기중";
    case "preflight_warning":
      return "경고";
    case "preflight_not_upload_ready":
      return "업로드 불가";
    case "rendering":
      return "내보내기 중";
    case "written_ok":
      return "내보내기 완료";
    case "written_with_warning":
      return "내보내기 완료";
    case "written_not_upload_ready":
      return "완료 / 업로드 불가";
    case "failed_to_render":
      return "내보내기 실패";
    case "optimized":
      return "최적화됨";
    case "cancelled":
      return "취소됨";
    case "pending":
    default:
      return "대기중";
  }
}

export function statusTone(status: ExportItemStatus) {
  if (status === "failed_to_render") {
    return "error";
  }
  if (isNotUploadReadyStatus(status)) {
    return "danger";
  }
  if (isWarningStatus(status)) {
    return "warning";
  }
  if (isWrittenStatus(status) || status === "preflight_ok") {
    return "ok";
  }
  if (status === "excluded") {
    return "muted";
  }

  return "neutral";
}

function isWrittenStatus(status: ExportItemStatus) {
  return (
    status === "written_ok" ||
    status === "written_with_warning" ||
    status === "written_not_upload_ready"
  );
}

function isPendingStatus(status: ExportItemStatus) {
  return (
    status === "pending" ||
    status === "preflight_ok" ||
    status === "preflight_warning" ||
    status === "preflight_not_upload_ready" ||
    status === "rendering" ||
    status === "optimized"
  );
}

function isUploadReadyStatus(status: ExportItemStatus) {
  return status === "preflight_ok" || status === "written_ok";
}

function isWarningStatus(status: ExportItemStatus) {
  return status === "preflight_warning" || status === "written_with_warning";
}

function isNotUploadReadyStatus(status: ExportItemStatus) {
  return (
    status === "preflight_not_upload_ready" ||
    status === "written_not_upload_ready"
  );
}

function pieceIdSet(issues: ExportValidationIssue[]) {
  return new Set(
    issues
      .map((issue) => issue.pieceId)
      .filter((pieceId): pieceId is string => Boolean(pieceId)),
  );
}

export function issueSummary(issues: ExportValidationIssue[]) {
  return issues.map((issue) => issue.message).join(" / ");
}

export interface MergeExportSessionOptions {
  dirtyIconIds?: Set<string>;
  dirtyPieceIds?: Set<string>;
  preserveNonDirtyExcluded?: boolean;
}

export function mergeExportSessionValidation(
  next: ExportValidationResult,
  previous: ExportValidationResult | null,
  options: MergeExportSessionOptions = {},
): ExportValidationResult {
  if (!previous) {
    return next;
  }

  const dirtyIconIds = options.dirtyIconIds ?? new Set<string>();
  const dirtyPieceIds = options.dirtyPieceIds ?? new Set<string>();
  const previousByPieceId = new Map(
    previous.items.map((item) => [item.pieceId, item] as const),
  );

  return {
    ...next,
    items: next.items.map((item) => {
      const previousItem = previousByPieceId.get(item.pieceId);
      if (
        !previousItem ||
        dirtyIconIds.has(item.iconId) ||
        dirtyPieceIds.has(item.pieceId)
      ) {
        return item;
      }

      if (!previousItem.included && !options.preserveNonDirtyExcluded) {
        return item;
      }

      if (isWrittenStatus(previousItem.status) || previousItem.status === "excluded") {
        return {
          ...item,
          byteSize: previousItem.byteSize,
          exportIndex: previousItem.exportIndex,
          exportPath: previousItem.exportPath,
          fileName: previousItem.fileName,
          included: previousItem.included,
          status: previousItem.status,
        };
      }

      return item;
    }),
  };
}
