import type {
  AiCandidate,
  AiManualServiceSurface,
  EffectiveVisualSource,
} from "@/features/editor/types";

export const AI_CANDIDATE_MAX_BYTES = 16 * 1024 * 1024;
export const AI_CANDIDATE_IMAGE_ACCEPT =
  ".jpg,.jpeg,.png,image/jpeg,image/png";
const AI_CANDIDATE_IMAGE_EXTENSIONS = new Set(["jpg", "jpeg", "png"]);

export const AI_MANUAL_SERVICE_OPTIONS: ReadonlyArray<{
  value: AiManualServiceSurface;
  label: string;
}> = [
  { value: "other_manual", label: "기타 수동/로컬 결과" },
  { value: "gemini_web", label: "Gemini 웹 결과 (수동)" },
  { value: "novelai_web", label: "NovelAI 웹 결과 (수동)" },
];

export function aiServiceSurfaceLabel(surface: AiManualServiceSurface) {
  return (
    AI_MANUAL_SERVICE_OPTIONS.find((option) => option.value === surface)?.label ??
    "수동/로컬 결과"
  );
}

export function activeAiSourceLabel(visualSource: EffectiveVisualSource) {
  return visualSource.activeVersionId ? "AI 소스 사용 중" : "원본 사용 중";
}

export function aiSourceActionLockReason(hasUnsavedChanges: boolean) {
  return hasUnsavedChanges
    ? "크롭·변형 또는 고급 편집 변경을 먼저 적용하거나 되돌려 주세요."
    : null;
}

export function aiCandidateFileFormatError(file: Pick<File, "name">) {
  const extension = file.name.split(".").pop()?.toLowerCase() ?? "";
  return AI_CANDIDATE_IMAGE_EXTENSIONS.has(extension)
    ? null
    : `${file.name}: 첫 AI 편집 단계에서는 JPG 또는 PNG 정적 이미지만 후보로 가져올 수 있습니다. GIF AI 편집은 프레임/스프라이트 실험 단계에서 추가 예정입니다.`;
}

export function aiCandidateFileSizeError(
  file: Pick<File, "name" | "size">,
) {
  return file.size > AI_CANDIDATE_MAX_BYTES
    ? `${file.name}: AI 후보 이미지는 최대 16MB까지 가져올 수 있습니다.`
    : null;
}

export function aiCandidateActionState(
  candidate: Pick<AiCandidate, "isMaterialized" | "isStale" | "staleReason">,
  isActive: boolean,
) {
  if (isActive) {
    return {
      disabled: true,
      label: "현재 아이콘에 적용됨",
      reason: "현재 편집 소스로 사용 중입니다.",
    };
  }
  if (candidate.isMaterialized) {
    return {
      disabled: true,
      label: "이미 적용된 후보",
      reason: "이미 적용한 후보입니다. 아래 AI 소스 이력에서 선택하세요.",
    };
  }
  if (candidate.isStale) {
    return {
      disabled: true,
      label: "현재 상태와 맞지 않음",
      reason:
        candidate.staleReason ??
        "후보를 가져온 뒤 원본 또는 편집 상태가 바뀌었습니다.",
    };
  }
  return {
    disabled: false,
    label: "현재 아이콘에 적용",
    reason: "원본을 보존한 채 이 후보를 현재 편집 소스로 적용합니다.",
  };
}

export function formatAiRecordedAt(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }

  return new Intl.DateTimeFormat("ko-KR", {
    dateStyle: "medium",
    timeStyle: "short",
  }).format(date);
}
