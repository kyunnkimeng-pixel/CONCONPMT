import { renderToString } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { MotionRecipeEditor } from "@/features/editor/components/MotionRecipeEditor";
import {
  emptyMotionRecipe,
  MOTION_PRESET_OPTIONS,
  setMotionPreset,
  type MotionCategory,
  type MotionPresetKind,
} from "@/features/editor/motion-recipe-model";
import type { MotionRecipeV1 } from "@/features/editor/types";

describe("MotionRecipeEditor", () => {
  it("renders four fixed categories in the native composition order", () => {
    const recipe = withRepresentativePresetInEveryCategory();
    const html = renderEditor(recipe);
    const text = plainText(html);
    const categoryLabels = [
      "1번째 모션 범주 공간 변형",
      "2번째 모션 범주 일렁임·변위",
      "3번째 모션 범주 색상·불투명도",
      "4번째 모션 범주 오버레이",
    ];

    expect(html).toContain('data-testid="motion-recipe-editor"');
    expect(html).toContain('aria-label="모션 고정 합성 순서"');
    for (const label of categoryLabels) {
      expect(html).toContain(`aria-label="${label}"`);
    }
    const categoryOffsets = categoryLabels.map((label) =>
      html.indexOf(`aria-label="${label}"`),
    );
    expect(
      categoryOffsets.every(
        (offset, index) =>
          offset >= 0 && (index === 0 || offset > categoryOffsets[index - 1]),
      ),
    ).toBe(true);
    expect(text).toContain(
      "공간 변형 → 일렁임·변위 → 색상·불투명도 → 오버레이 순서로",
    );
    expect(text).not.toMatch(/위로 이동|아래로 이동|효과 추가/);
  });

  it("offers every implemented motion preset without future placeholders", () => {
    const html = renderEditor(withRepresentativePresetInEveryCategory());
    const text = plainText(html);

    for (const options of Object.values(MOTION_PRESET_OPTIONS)) {
      for (const option of options) {
        expect(text).toContain(option.label);
      }
    }
    expect(text).toContain("모든 모션 끄기");
    expect(text).toContain("이 범주 기본값");
    expect(text).not.toContain("준비 중");
  });

  it("lets static sources set duration and FPS and explains GIF conversion", () => {
    const html = renderEditor(
      setMotionPreset(emptyMotionRecipe(), "spatial", "shake"),
    );
    const text = plainText(html);

    expect(text).toContain("정적 이미지는 아래 길이와 FPS로 새 GIF가 됩니다.");
    expect(text).toContain("한 루프 길이");
    expect(text).toContain("GIF FPS");
    expect(text).toContain("예상 프레임");
    expect(text).toContain("12개");
    expect(text).toContain("출력 형식");
    expect(text).toContain("GIF");
    expect(html).toContain('aria-label="한 루프 길이 숫자 입력"');
    expect(html).toContain('aria-label="GIF FPS 숫자 입력"');
  });

  it("explains static output and hides inactive GIF timing when no motion is enabled", () => {
    const html = renderEditor(emptyMotionRecipe());
    const text = plainText(html);

    expect(text).toContain(
      "활성 모션 없음 · 현재 정적 출력 형식을 유지합니다.",
    );
    expect(text).toContain("활성 모션 없음 · 정적 출력");
    expect(text).toContain(
      "모션을 하나 이상 켜면 보간 방식과 가장자리 처리를 적용할 수 있습니다.",
    );
    expect(text).not.toContain("GIF FPS");
    expect(text).not.toContain("예상 프레임");
    expect(html).toMatch(/<button[^>]*disabled=""[^>]*>패턴 바꾸기<\/button>/);
    expect((html.match(/<select[^>]*disabled=""/g) ?? []).length).toBeGreaterThanOrEqual(2);
  });

  it("shows measured GIF timing read-only instead of editable duration controls", () => {
    const html = renderEditor(emptyMotionRecipe(), {
      isAnimatedSource: true,
      measuredDurationMs: 1_240,
      measuredFps: 11.5,
      sourceFrameCount: 14,
    });
    const text = plainText(html);

    expect(text).toContain(
      "원본 GIF의 실제 프레임 시간을 유지하고 누적 시간으로 모션 위상을 계산합니다.",
    );
    expect(text).toContain("원본 프레임");
    expect(text).toContain("14개");
    expect(text).toContain("1.24초");
    expect(text).toContain("11.50fps");
    expect(html).not.toContain('aria-label="한 루프 길이 숫자 입력"');
    expect(html).not.toContain('aria-label="GIF FPS 숫자 입력"');
  });

  it.each([
    ["spatial", "shake", ["루프당 횟수", "가로 진폭", "세로 진폭"]],
    ["spatial", "bounce", ["루프당 횟수", "튀는 높이"]],
    ["spatial", "breathe", ["루프당 횟수", "크기 변화"]],
    ["spatial", "rock", ["루프당 횟수", "회전 각도"]],
    ["spatial", "spin", ["루프당 횟수", "회전 방향"]],
    ["displacement", "wave", ["물결 방향", "진폭", "파장"]],
    [
      "displacement",
      "jelly",
      ["가로 진폭", "세로 진폭", "가로 파장", "세로 파장"],
    ],
    ["displacement", "ripple", ["진폭", "파장", "중심 X", "중심 Y"]],
    [
      "displacement",
      "glitchBands",
      ["가로 이동", "밴드 높이", "단계 수"],
    ],
    ["colorOpacity", "hueCycle", ["색조 범위"]],
    ["colorOpacity", "tintPulse", ["지정색", "최대 혼합"]],
    [
      "colorOpacity",
      "brightnessSaturationPulse",
      ["밝기 변화", "채도 변화"],
    ],
    ["colorOpacity", "flash", ["번쩍임 색", "강도"]],
    [
      "overlay",
      "focusLines",
      ["선 색", "선 개수", "선 두께", "안쪽 반경", "불투명도"],
    ],
    ["overlay", "sparkle", ["반짝이 색", "개수", "크기", "불투명도"]],
    [
      "overlay",
      "expansionRing",
      ["링 색", "선 두께", "최대 반경", "불투명도"],
    ],
  ] as const)(
    "renders labeled parameters for %s/%s",
    (category, kind, labels) => {
      const recipe = setMotionPreset(
        emptyMotionRecipe(),
        category,
        kind,
      );
      const text = plainText(renderEditor(recipe));

      for (const label of labels) {
        expect(text).toContain(label);
      }
      expect(text).toContain("루프당 횟수");
      expect(text).toContain("보간 방식");
      expect(text).toContain("가장자리 처리");
    },
  );
});

function withRepresentativePresetInEveryCategory() {
  return [
    ["spatial", "shake"],
    ["displacement", "wave"],
    ["colorOpacity", "hueCycle"],
    ["overlay", "focusLines"],
  ].reduce<MotionRecipeV1>(
    (recipe, [category, kind]) =>
      setMotionPreset(
        recipe,
        category as MotionCategory,
        kind as MotionPresetKind,
      ),
    emptyMotionRecipe(),
  );
}

function renderEditor(
  recipe: MotionRecipeV1,
  overrides: Partial<{
    isAnimatedSource: boolean;
    measuredDurationMs: number | null;
    measuredFps: number | null;
    sourceFrameCount: number | null;
  }> = {},
) {
  return renderToString(
    <MotionRecipeEditor
      isAnimatedSource={overrides.isAnimatedSource ?? false}
      measuredDurationMs={overrides.measuredDurationMs ?? null}
      measuredFps={overrides.measuredFps ?? null}
      recipe={recipe}
      sourceFrameCount={overrides.sourceFrameCount ?? null}
      onChange={() => undefined}
    />,
  );
}

function plainText(html: string) {
  return html.replace(/<!-- -->/g, "");
}
