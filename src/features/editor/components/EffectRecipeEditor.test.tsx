import { renderToString } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { EffectRecipeEditor } from "@/features/editor/components/EffectRecipeEditor";
import {
  addEffect,
  emptyEffectRecipe,
} from "@/features/editor/effect-recipe-model";
import type {
  EffectRecipeV1,
  IconEffect,
} from "@/features/editor/types";

describe("EffectRecipeEditor", () => {
  it("renders an accessible ordered Korean effect editor without future motion actions", () => {
    const recipe = addEffect(
      addEffect(emptyEffectRecipe(), "pixelate"),
      "shadow",
    );
    const html = renderEditor(recipe);
    const text = html.replace(/<!-- -->/g, "");

    expect(html).toContain('data-testid="effect-recipe-editor"');
    expect(html).toContain('aria-label="효과 적용 순서"');
    expect(html).toContain('aria-label="1번째 효과 픽셀화"');
    expect(html).toContain('aria-label="2번째 효과 그림자"');
    expect(html).toContain('aria-label="픽셀화 사용"');
    expect(html).toContain('aria-label="픽셀화 위로 이동"');
    expect(html).toContain('aria-label="픽셀화 아래로 이동"');
    expect(html).toContain('aria-label="그림자 제거"');
    expect(text).toContain("위에서 아래 순서로");
    expect(text).toContain("2개 사용");
    expect(text).not.toMatch(/흔들기|일렁임|펄스|움직이는 효과/);
  });

  it("offers only implemented curated effect kinds in the add selector", () => {
    const html = renderEditor(emptyEffectRecipe());
    const text = html.replace(/<!-- -->/g, "");

    for (const label of [
      "픽셀화",
      "색상 조정",
      "색감 프리셋",
      "블러",
      "선명화",
      "윤곽선",
      "그림자",
    ]) {
      expect(text).toContain(label);
    }
    expect(text).toContain("효과 추가");
    expect(text).toContain("적용할 효과가 없습니다");
    expect(text).toContain("0/16단계");
  });

  it("disables adding and explains the limit at 16 stages", () => {
    const recipe = Array.from({ length: 16 }).reduce<EffectRecipeV1>(
      (current) => addEffect(current, "blur"),
      emptyEffectRecipe(),
    );
    const html = renderEditor(recipe);
    const text = html.replace(/<!-- -->/g, "");

    expect(text).toContain("16/16단계 · 최대 단계에 도달했습니다.");
    expect(html).toMatch(/<select[^>]*disabled=""/);
    expect(html).toMatch(/<button[^>]*disabled=""[^>]*>효과 추가<\/button>/);
  });

  it.each([
    [
      {
        id: "pixel",
        kind: "pixelate",
        enabled: true,
        blockSize: 4,
      } satisfies IconEffect,
      ["블록 크기", "픽셀화 사용"],
    ],
    [
      {
        id: "color",
        kind: "color_adjust",
        enabled: true,
        brightness: 0,
        contrast: 0,
        saturation: 0,
        hue: 0,
      } satisfies IconEffect,
      ["밝기", "대비", "채도", "색조"],
    ],
    [
      {
        id: "tone",
        kind: "tone",
        enabled: true,
        mode: "sepia",
        amount: 80,
      } satisfies IconEffect,
      ["색감", "흑백", "세피아", "강도"],
    ],
    [
      {
        id: "blur",
        kind: "blur",
        enabled: true,
        radius: 2,
      } satisfies IconEffect,
      ["블러 반경"],
    ],
    [
      {
        id: "sharpen",
        kind: "sharpen",
        enabled: true,
        amount: 25,
      } satisfies IconEffect,
      ["선명화 강도"],
    ],
    [
      {
        id: "outline",
        kind: "outline",
        enabled: true,
        radius: 2,
        color: "#ffffff",
      } satisfies IconEffect,
      ["윤곽선 두께", "윤곽선 색"],
    ],
    [
      {
        id: "shadow",
        kind: "shadow",
        enabled: true,
        offsetX: 4,
        offsetY: 4,
        blurRadius: 4,
        color: "#000000",
      } satisfies IconEffect,
      ["가로 거리", "세로 거리", "그림자 흐림", "그림자 색"],
    ],
  ])("renders labeled parameters for %#", (effect, labels) => {
    const html = renderEditor({ version: 1, effects: [effect] });

    for (const label of labels) {
      expect(html).toContain(label);
    }
    expect(html).toContain("매개변수 기본값");
    expect(html).toContain('aria-expanded="true"');
  });

  it("communicates exact reset scope and disables every action when read-only", () => {
    const recipe = addEffect(emptyEffectRecipe(), "outline");
    const html = renderEditor(recipe, true);
    const text = html.replace(/<!-- -->/g, "");

    expect(text).toContain("모든 효과 끄기");
    expect(text).toContain("매개변수 기본값");
    expect(html).toContain(
      'title="효과 항목과 매개변수는 유지하고 사용 상태만 끕니다."',
    );
    expect(html).toContain(
      'title="이 효과의 매개변수만 권장 기본값으로 되돌립니다."',
    );
    expect(html).not.toContain(">초기화<");
    expect((html.match(/disabled=""/g) ?? []).length).toBeGreaterThan(5);
  });
});

function renderEditor(recipe: EffectRecipeV1, disabled = false) {
  return renderToString(
    <EffectRecipeEditor
      disabled={disabled}
      recipe={recipe}
      onChange={() => undefined}
    />,
  );
}
