import { describe, expect, it } from "vitest";

import {
  buildNovelAiGuideSpec,
  needsNovelAiEnglishInputHint,
  normalizeNovelAiPromptInput,
  novelAiUndesiredContentForTask,
} from "@/features/ai-web/novelai-web-model";

describe("NovelAI manual web guidance model", () => {
  it("normalizes user tags to lower-case comma-separated input without translating Korean", () => {
    expect(
      normalizeNovelAiPromptInput("  Happy FACE;\nblue   EYES, 표정 유지  "),
    ).toBe("happy face, blue eyes, 표정 유지");
    expect(needsNovelAiEnglishInputHint("표정 유지")).toBe(true);
    expect(needsNovelAiEnglishInputHint("happy face, blue eyes")).toBe(false);
  });

  it("separates single-image undesired tags from sheet structure tags", () => {
    expect(novelAiUndesiredContentForTask("single_edit")).toContain(
      "multiple views",
    );
    expect(novelAiUndesiredContentForTask("gif_frame_sheet")).toContain(
      "merged cells",
    );
    expect(novelAiUndesiredContentForTask("gif_frame_sheet")).not.toContain(
      "multiple views",
    );
    for (const task of [
      "single_edit",
      "grid_edit",
      "grid_generate",
      "gif_frame_sheet",
    ] as const) {
      const undesired = novelAiUndesiredContentForTask(task);
      expect(undesired).toContain("checkerboard");
      expect(undesired).toContain("checkered background");
      expect(undesired).toContain("fake transparency");
      expect(undesired).toContain("opaque background");
    }
  });

  it("recommends Image2Image for exact layouts and explains 200 to 192 normalization only for single icons", () => {
    const single = buildNovelAiGuideSpec({
      task: "single_edit",
      expectedCanvas: "200×200px",
      allowsProportionalNormalization: true,
    });
    const gif = buildNovelAiGuideSpec({
      task: "gif_frame_sheet",
      expectedCanvas: "1024×1024px",
    });

    expect(single.recommendedMode).toContain("Image2Image");
    expect(single.resolutionText).toContain("192×192");
    expect(single.resolutionText).toContain("200×200px로 맞춥니다");
    expect(single.steps.join(" ")).toContain("Add a Base Img (Optional)");
    expect(single.steps.join(" ")).toContain("What do you want to do with this image?");
    expect(single.steps.join(" ")).toContain("Prompt 입력란에 먼저");
    expect(single.steps.join(" ")).toContain("Undesired Content 입력란");
    expect(single.steps.join(" ")).toContain("아래쪽 별도 필드");
    expect(gif.recommendedMode).toContain("Image2Image");
    expect(gif.resolutionText).toContain("정확히 유지");
    expect(gif.resolutionText).not.toContain("192×192");
    expect(gif.steps.join(" ")).toContain("Account Settings를 연 뒤 Image Settings 탭 → Image Generation → Image Format for Generated Images");
    expect(gif.steps.join(" ")).toContain("Download Image");
    expect(gif.steps.join(" ")).toContain("Remove BG");
    expect(gif.steps.join(" ")).toContain("한 장씩 Add a Base Img (Optional)");
    expect(gif.steps.join(" ")).toContain("해상도는 앱이 각 페이지에 표시한 실제 캔버스");
    expect(gif.steps.join(" ")).toContain("페이지별 슬롯");
    expect(gif.warningText).toContain("다운로드명을 바꾸면");
  });

  it("keeps fake transparency forbidden without forcing transparency in opaque mode", () => {
    const undesired = novelAiUndesiredContentForTask(
      "grid_generate",
      "allow_opaque",
    );
    const spec = buildNovelAiGuideSpec({
      task: "grid_generate",
      expectedCanvas: "1024×1024px",
      backgroundPolicy: "allow_opaque",
    });

    expect(undesired).not.toContain("opaque background");
    expect(undesired).toContain("checkerboard");
    expect(undesired).toContain("fake transparency");
    expect(spec.steps.join(" ")).not.toContain("Remove BG");
    expect(spec.steps.join(" ")).toContain("배경 제거 도구가 필수가 아닙니다");
    expect(spec.steps.join(" ")).toContain("체커무늬·가짜 투명 패턴");
  });

  it("distinguishes reference generation modes and their incompatibility", () => {
    const spec = buildNovelAiGuideSpec({
      task: "grid_generate",
      expectedCanvas: "1024×1024px",
      hasReference: true,
    });

    expect(spec.recommendedMode).toContain("Vibe Transfer");
    expect(spec.recommendedMode).toContain("Precise Reference");
    expect(spec.steps.join(" ")).toContain("동시에 사용하지 않습니다");
    expect(spec.warningText).toContain("Anlas");
  });
});
