import type {
  AiWebHandoffValidationIssue,
  AiWebHandoffValidationIssueCode,
} from "@/features/editor/types";

export const AI_WEB_HANDOFF_RESULT_ACCEPT =
  ".jpg,.jpeg,.png,image/jpeg,image/png";
export const AI_WEB_HANDOFF_RESULT_MAX_BYTES = 16 * 1024 * 1024;

export interface AiWebHandoffIssueGuidance {
  problem: string;
  impact: string;
  fix: string;
  correctionPrompt: string | null;
}

export interface AiWebErrorGuidance {
  category: "authentication" | "quota" | "network" | "file" | "unknown";
  title: string;
  action: string;
  correctionPrompt: null;
}

const ISSUE_CODE_ALIASES: Readonly<
  Record<string, AiWebHandoffValidationIssueCode>
> = {
  ai_handoff_result_dimensions: "canvas_size_mismatch",
  ai_handoff_result_alpha_lost: "transparency_lost",
  ai_handoff_result_corrupt: "decode_failed",
  ai_handoff_result_format: "unsupported_format",
  ai_handoff_result_too_large: "file_too_large",
  ai_handoff_result_animated: "unsupported_format",
  ai_handoff_result_stale: "source_state_changed",
  ai_handoff_result_signature_mismatch: "source_state_changed",
  wrong_canvas_size: "canvas_size_mismatch",
  alpha_lost: "transparency_lost",
  grid_item_count_mismatch: "item_count_mismatch",
  cell_count_mismatch: "item_count_mismatch",
  wrong_grid: "grid_geometry_mismatch",
  wrong_frame_count: "frame_count_mismatch",
  expired: "handoff_expired",
  stale_source: "source_state_changed",
};

function canonicalIssueCode(
  code: string,
): AiWebHandoffValidationIssueCode | "unknown" {
  const normalized = code.trim().toLowerCase();
  return (
    ISSUE_CODE_ALIASES[normalized] ??
    (
      [
        "unsupported_format",
        "decode_failed",
        "file_too_large",
        "canvas_size_mismatch",
        "transparency_lost",
        "page_count_mismatch",
        "item_count_mismatch",
        "grid_geometry_mismatch",
        "frame_count_mismatch",
        "source_state_changed",
        "handoff_expired",
        "result_missing",
      ] as const
    ).find((value) => value === normalized) ??
    "unknown"
  );
}

function expectedActualSuffix(issue: AiWebHandoffValidationIssue) {
  const expected = issue.expected?.trim();
  const actual = issue.actual?.trim();
  if (expected && actual) {
    return ` 예상값은 ${expected}, 현재 결과는 ${actual}입니다.`;
  }
  if (expected) {
    return ` 예상값은 ${expected}입니다.`;
  }
  return "";
}

function describeKnownAiWebHandoffIssue(
  issue: AiWebHandoffValidationIssue,
): AiWebHandoffIssueGuidance {
  const suffix = expectedActualSuffix(issue);
  switch (canonicalIssueCode(issue.code)) {
    case "unsupported_format":
      return {
        problem: issue.message || "지원하지 않는 결과 파일 형식입니다.",
        impact: "결과 이미지를 안전하게 읽고 후보로 보관할 수 없습니다.",
        fix: "웹에서 결과를 JPG 또는 PNG 파일로 내려받아 다시 놓아 주세요.",
        correctionPrompt: null,
      };
    case "decode_failed":
      return {
        problem: issue.message || "결과 이미지 파일을 읽을 수 없습니다.",
        impact: "손상되었거나 이미지가 아닌 파일은 후보로 보관할 수 없습니다.",
        fix: "웹에서 결과를 다시 내려받거나 다른 JPG·PNG 파일을 선택해 주세요.",
        correctionPrompt: null,
      };
    case "file_too_large":
      return {
        problem: issue.message || "결과 이미지가 허용 용량을 초과했습니다.",
        impact:
          "앱의 안전한 이미지 처리 한도를 넘어 결과를 가져오지 않았습니다.",
        fix: "웹에서 더 작은 해상도나 JPG·PNG 결과로 다시 저장해 주세요.",
        correctionPrompt: null,
      };
    case "canvas_size_mismatch": {
      const expected = issue.expected?.trim() || "처음 전달한 크기";
      return {
        problem: `${issue.message || "결과 캔버스 크기가 달라졌습니다."}${suffix}`,
        impact: "셀과 프레임 경계를 원래 위치에 정확하게 연결할 수 없습니다.",
        fix: "처음 전달한 캔버스 크기를 유지해 결과를 다시 만들어 주세요.",
        correctionPrompt: `출력 캔버스 크기를 정확히 ${expected}로 유지하고 이미지의 가로세로 크기를 변경하지 마세요.`,
      };
    }
    case "transparency_lost":
      return {
        problem: issue.message || "결과에서 투명 배경이 사라졌습니다.",
        impact: "아이콘 주변 배경이 불투명하게 저장될 수 있습니다.",
        fix: "투명 배경과 alpha 채널을 유지한 PNG로 다시 만들어 주세요.",
        correctionPrompt:
          "투명 배경을 유지하고 alpha 채널이 포함된 PNG 한 장으로 반환해 주세요.",
      };
    case "page_count_mismatch": {
      const expected = issue.expected?.trim() || "1페이지";
      return {
        problem: `${issue.message || "결과 시트 페이지 수가 다릅니다."}${suffix}`,
        impact: "일부 셀이나 프레임이 빠졌을 수 있어 자동 연결하지 않았습니다.",
        fix: "페이지를 나누거나 추가하지 말고 처음 구조 그대로 다시 만들어 주세요.",
        correctionPrompt: `결과를 정확히 ${expected}의 이미지 시트로 반환하고 페이지를 추가하거나 나누지 마세요.`,
      };
    }
    case "item_count_mismatch": {
      const expected = issue.expected?.trim() || "처음 전달한 개수";
      return {
        problem: `${issue.message || "결과 아이콘 수가 다릅니다."}${suffix}`,
        impact: "결과 셀을 원래 아이콘에 안전하게 대응시킬 수 없습니다.",
        fix: "아이콘을 추가·삭제·복제하지 않고 같은 개수로 다시 만들어 주세요.",
        correctionPrompt: `아이콘을 추가·삭제·복제하지 말고 정확히 ${expected}를 유지해 주세요.`,
      };
    }
    case "grid_geometry_mismatch": {
      const expected = issue.expected?.trim() || "처음 전달한 행·열과 셀 경계";
      return {
        problem: `${issue.message || "결과의 그리드 구조가 달라졌습니다."}${suffix}`,
        impact: "셀 경계가 어긋나 결과를 잘못 잘라낼 수 있습니다.",
        fix: "행·열, 셀 간격과 셀 경계를 바꾸지 않고 다시 만들어 주세요.",
        correctionPrompt: `그리드를 정확히 ${expected}로 유지하고 셀을 합치거나 이동하거나 경계를 넘지 마세요.`,
      };
    }
    case "frame_count_mismatch": {
      const expected = issue.expected?.trim() || "처음 전달한 프레임 수";
      return {
        problem: `${issue.message || "결과 프레임 수가 다릅니다."}${suffix}`,
        impact: "원래 재생 시간과 프레임 순서로 GIF를 재조립할 수 없습니다.",
        fix: "프레임을 추가·삭제·복제하지 않고 같은 수로 다시 만들어 주세요.",
        correctionPrompt: `프레임을 추가·삭제·복제하지 말고 정확히 ${expected}를 같은 순서로 유지해 주세요.`,
      };
    }
    case "source_state_changed":
      return {
        problem:
          issue.message || "전달 준비 후 원본 또는 편집 상태가 바뀌었습니다.",
        impact:
          "이 결과를 현재 아이콘에 연결하면 다른 버전과 섞일 수 있습니다.",
        fix: "현재 상태에서 웹 전달을 새로 준비해 주세요.",
        correctionPrompt: null,
      };
    case "handoff_expired":
      return {
        problem: issue.message || "웹 전달 준비의 보관 기간이 끝났습니다.",
        impact: "내부 구조 정보가 없어 결과를 안전하게 연결할 수 없습니다.",
        fix: "웹 전달을 새로 준비하거나, 아직 가능한 경우 보관 기간을 한 번 연장해 주세요.",
        correctionPrompt: null,
      };
    case "result_missing":
      return {
        problem: issue.message || "가져올 결과 이미지가 없습니다.",
        impact: "검사하거나 후보로 보관할 파일이 없습니다.",
        fix: "웹에서 JPG·PNG 결과를 내려받아 결과 영역에 놓아 주세요.",
        correctionPrompt: null,
      };
    default:
      return {
        problem: issue.message || "결과를 안전하게 확인할 수 없습니다.",
        impact:
          "확인되지 않은 결과는 원본과 후보 이력을 보호하기 위해 가져오지 않았습니다.",
        fix: "표시된 문제를 확인한 뒤 결과를 다시 내려받거나 웹 전달을 새로 준비해 주세요.",
        correctionPrompt: null,
      };
  }
}

export function describeAiWebHandoffIssue(
  issue: AiWebHandoffValidationIssue,
): AiWebHandoffIssueGuidance {
  const guidance = describeKnownAiWebHandoffIssue(issue);
  const localAction = issue.localAction?.trim();
  const suggestedPrompt = issue.suggestedPrompt?.trim();
  return {
    ...guidance,
    fix: localAction || guidance.fix,
    correctionPrompt: suggestedPrompt || guidance.correctionPrompt,
  };
}

export function buildCombinedAiWebHandoffCorrectionPrompt(
  issues: ReadonlyArray<AiWebHandoffValidationIssue>,
) {
  const corrections = Array.from(
    new Set(
      issues
        .map((issue) => describeAiWebHandoffIssue(issue).correctionPrompt)
        .filter((value): value is string => Boolean(value)),
    ),
  );
  if (corrections.length === 0) {
    return null;
  }
  return `[구조 수정 요청]\n${corrections.map((value) => `- ${value}`).join("\n")}`;
}

export function selectAiWebHandoffResultFile(
  files: Iterable<File> | ArrayLike<File>,
) {
  const items = Array.from(files as ArrayLike<File>);
  if (items.length === 0) {
    return {
      file: null,
      error:
        "웹페이지의 미리보기 주소가 아니라 내려받은 JPG·PNG 파일을 놓아 주세요.",
    };
  }
  if (items.length !== 1) {
    return {
      file: null,
      error: "현재 전달 결과는 JPG 또는 PNG 파일 한 장만 가져올 수 있습니다.",
    };
  }
  const file = items[0]!;
  const extension = file.name.split(".").pop()?.toLowerCase() ?? "";
  if (!["jpg", "jpeg", "png"].includes(extension)) {
    return {
      file: null,
      error: `${file.name}: JPG 또는 PNG 결과 파일만 가져올 수 있습니다.`,
    };
  }
  if (file.size > AI_WEB_HANDOFF_RESULT_MAX_BYTES) {
    return {
      file: null,
      error: `${file.name}: 결과 이미지는 최대 16MB까지 가져올 수 있습니다.`,
    };
  }
  return { file, error: null };
}

export function classifyPastedWebError(
  value: string,
): AiWebErrorGuidance | null {
  const text = value.trim();
  if (!text) return null;
  const normalized = text.toLocaleLowerCase();
  if (
    /(401|unauthori[sz]ed|invalid (api )?key|sign[ -]?in|log[ -]?in|로그인|인증|키가.+거부)/i.test(
      normalized,
    )
  ) {
    return {
      category: "authentication",
      title: "로그인 또는 인증 문제로 보입니다.",
      action:
        "웹사이트에서 로그인 상태와 계정 권한을 확인한 뒤 사용자가 직접 다시 시도해 주세요.",
      correctionPrompt: null,
    };
  }
  if (
    /(429|rate.?limit|quota|usage.?limit|credit|anlas|할당량|사용량|크레딧|요청.+제한)/i.test(
      normalized,
    )
  ) {
    return {
      category: "quota",
      title: "사용량 또는 요청 제한으로 보입니다.",
      action:
        "웹사이트의 남은 사용량과 제한 해제 시점을 확인한 뒤 사용자가 직접 다시 시도해 주세요.",
      correctionPrompt: null,
    };
  }
  if (
    /(network|connection|timeout|timed out|offline|dns|네트워크|연결|시간.+초과)/i.test(
      normalized,
    )
  ) {
    return {
      category: "network",
      title: "네트워크 연결 문제로 보입니다.",
      action:
        "인터넷 연결과 웹사이트 상태를 확인하세요. 앱은 자동으로 다시 요청하지 않습니다.",
      correctionPrompt: null,
    };
  }
  if (
    /(unsupported.+(file|format)|invalid.+(image|file)|file.+too large|지원하지.+파일|파일.+형식|용량)/i.test(
      normalized,
    )
  ) {
    return {
      category: "file",
      title: "업로드 파일 조건 문제로 보입니다.",
      action:
        "전달 도우미가 만든 파일을 그대로 올렸는지 확인하고, 필요하면 웹 전달을 새로 준비해 주세요.",
      correctionPrompt: null,
    };
  }
  return {
    category: "unknown",
    title: "웹사이트 오류 유형을 확정하지 못했습니다.",
    action:
      "오류 원문을 보존한 채 해당 웹사이트의 도움말을 확인하세요. 앱은 프롬프트를 추측하거나 자동 재시도하지 않습니다.",
    correctionPrompt: null,
  };
}
