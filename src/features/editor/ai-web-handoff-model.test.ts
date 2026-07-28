import { describe, expect, it } from "vitest";

import {
  buildCombinedAiWebHandoffCorrectionPrompt,
  classifyPastedWebError,
  describeAiWebHandoffIssue,
  selectAiWebHandoffResultFile,
} from "@/features/editor/ai-web-handoff-model";
import type { AiWebHandoffValidationIssue } from "@/features/editor/types";

function issue(
  input: Partial<AiWebHandoffValidationIssue> &
    Pick<AiWebHandoffValidationIssue, "code">,
): AiWebHandoffValidationIssue {
  return {
    code: input.code,
    severity: input.severity ?? "blocking",
    message: input.message ?? "",
    expected: input.expected ?? null,
    actual: input.actual ?? null,
  };
}

describe("AI web handoff result diagnostics", () => {
  it("describes proportional size normalization as a local non-blocking action", () => {
    const guidance = describeAiWebHandoffIssue(
      issue({
        code: "ai_handoff_result_size_normalization",
        severity: "warning",
        message: "1024×1024px 결과를 200×200px로 정규화합니다.",
      }),
    );

    expect(guidance.problem).toContain("1024×1024px");
    expect(guidance.impact).toContain("원본 해상도는 후보로 보존");
    expect(guidance.correctionPrompt).toBeNull();
  });

  it("builds deterministic, de-duplicated correction text only for prompt-fixable issues", () => {
    const issues = [
      issue({
        code: "canvas_size_mismatch",
        expected: "1024×1024",
        actual: "896×1024",
      }),
      issue({
        code: "canvas_size_mismatch",
        expected: "1024×1024",
        actual: "800×800",
      }),
      issue({ code: "transparency_lost" }),
      issue({ code: "decode_failed" }),
    ];

    expect(buildCombinedAiWebHandoffCorrectionPrompt(issues)).toBe(
      [
        "[구조 수정 요청]",
        "- 출력 캔버스 크기를 정확히 1024×1024로 유지하고 이미지의 가로세로 크기를 변경하지 마세요.",
        "- 투명 배경을 유지하고 alpha 채널이 포함된 PNG 한 장으로 반환해 주세요.",
      ].join("\n"),
    );
  });

  it("shows expected and actual values while keeping technical failures action-only", () => {
    const layout = describeAiWebHandoffIssue(
      issue({
        code: "grid_geometry_mismatch",
        message: "행과 열이 바뀌었습니다.",
        expected: "3×3",
        actual: "2×4",
      }),
    );
    const expired = describeAiWebHandoffIssue(
      issue({ code: "handoff_expired" }),
    );

    expect(layout.problem).toContain("예상값은 3×3");
    expect(layout.problem).toContain("현재 결과는 2×4");
    expect(layout.correctionPrompt).toContain("3×3");
    expect(expired.correctionPrompt).toBeNull();
    expect(expired.fix).toContain("새로 준비");
  });

  it("does not invent a correction prompt for unknown backend issue codes", () => {
    const guidance = describeAiWebHandoffIssue(
      issue({
        code: "future_semantic_check",
        message: "사람의 검토가 필요합니다.",
        severity: "manual_review",
      }),
    );

    expect(guidance.problem).toBe("사람의 검토가 필요합니다.");
    expect(guidance.correctionPrompt).toBeNull();
  });
});

describe("AI web handoff result file selection", () => {
  it("accepts one local JPG or PNG and rejects missing, multiple, unsupported, and oversized files", () => {
    const png = { name: "result.png", size: 100, type: "image/png" } as File;
    const jpg = { name: "result.JPEG", size: 100, type: "image/jpeg" } as File;
    const gif = { name: "result.gif", size: 100, type: "image/gif" } as File;
    const large = {
      name: "large.png",
      size: 16 * 1024 * 1024 + 1,
      type: "image/png",
    } as File;

    expect(selectAiWebHandoffResultFile([png])).toEqual({
      file: png,
      error: null,
    });
    expect(selectAiWebHandoffResultFile([jpg]).error).toBeNull();
    expect(selectAiWebHandoffResultFile([]).error).toContain("내려받은");
    expect(selectAiWebHandoffResultFile([png, jpg]).error).toContain("한 장");
    expect(selectAiWebHandoffResultFile([gif]).error).toContain("JPG 또는 PNG");
    expect(selectAiWebHandoffResultFile([large]).error).toContain("16MB");
  });
});

describe("pasted web error guidance", () => {
  it.each([
    ["401 Unauthorized", "authentication"],
    ["Quota exceeded (429)", "quota"],
    ["Network timeout", "network"],
    ["Unsupported file format", "file"],
    ["provider did something unfamiliar", "unknown"],
  ] as const)("classifies %s as %s without generating prompt text", (text, category) => {
    const guidance = classifyPastedWebError(text);

    expect(guidance?.category).toBe(category);
    expect(guidance?.correctionPrompt).toBeNull();
  });

  it("returns no guidance for blank pasted text", () => {
    expect(classifyPastedWebError(" \n ")).toBeNull();
  });
});
