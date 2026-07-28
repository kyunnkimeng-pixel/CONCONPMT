import { renderToString } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { AiProviderPanel } from "@/features/editor/components/AiProviderPanel";
import type {
  CollectionSummary,
  IconSummary,
} from "@/features/collections/types";
import type { SourceFileSummary } from "@/features/editor/types";

const collection = {
  id: "collection_1",
  defaultCellWidth: 200,
  defaultCellHeight: 200,
} as CollectionSummary;

const icon = {
  id: "icon_1",
  collectionId: collection.id,
  displayName: "테스트 GIF",
  cellWidthOverride: null,
  cellHeightOverride: null,
} as IconSummary;

const source: SourceFileSummary = {
  id: "source_1",
  originalFilename: "durable-base.png",
  originalImageUrl: "asset://durable-base.png",
  originalExtension: "png",
  mimeType: "image/png",
  sha256: "a".repeat(64),
  hasAlpha: true,
  width: 640,
  height: 480,
  byteSize: 1234,
  isAnimated: false,
  frameCount: null,
  originalLoopMode: "preserve",
  originalLoopCount: null,
};

function renderPanel(
  initialProviderChoice: "novelai" | "gemini" | "web" = "novelai",
  currentSource = source,
) {
  return renderToString(
    <AiProviderPanel
      collection={collection}
      disabled={false}
      hasUnsavedChanges
      icon={icon}
      initialProviderChoice={initialProviderChoice}
      source={currentSource}
      onAnnouncement={() => {}}
      onBusyEnd={() => {}}
      onBusyStart={() => true}
      onGenerated={() => {}}
    />,
  );
}

function openingTag(html: string, testId: string) {
  const marker = `data-testid="${testId}"`;
  const markerIndex = html.indexOf(marker);
  if (markerIndex < 0) return "";
  const start = html.lastIndexOf("<", markerIndex);
  const end = html.indexOf(">", markerIndex);
  return html.slice(start, end + 1);
}

describe("AiProviderPanel credential and source UX", () => {
  it("renders an uncontrolled password field with no secret value or duplicate live region", () => {
    const html = renderPanel();
    const input = openingTag(html, "ai-novelai-credential");

    expect(input).toContain('type="password"');
    expect(input).toContain('autoComplete="off"');
    expect(input).not.toContain("value=");
    expect(html).not.toContain("pst-secret-never-echo");
    expect(html).not.toContain("aria-live=");
    expect(html).toContain("Rust 메모리에만");
    expect(html).toContain("호출 전에 즉시 비워");
  });

  it("states the exact durable base source and that unsaved/final rendering is not sent", () => {
    const html = renderPanel();

    expect(html).toContain("durable-base.png");
    expect(html).toContain("640");
    expect(html).toContain("480");
    expect(html).toContain("저장된 기준 소스");
    expect(html).toContain("최종 렌더 미리보기는 전송하지 않습니다.");
    expect(html).toContain("저장하지 않은 편집이 있어");
  });

  it("offers the manual GIF frame-sheet roundtrip without implying direct provider API support", () => {
    const html = renderPanel("novelai", {
      ...source,
      originalFilename: "animated.gif",
      originalExtension: "gif",
      mimeType: "image/gif",
      isAnimated: true,
      frameCount: 12,
    });

    expect(html).toContain('data-testid="ai-gif-frame-sheet-entry"');
    expect(html).toContain("GIF 프레임 시트 AI 왕복");
    expect(html).toContain("직접 GIF API 호출이나 자동 업로드는 하지 않습니다.");
    expect(html).toContain("원본 GIF와 프레임별 timing·loop");
    expect(html).not.toContain("다음 업데이트");
    expect(html).not.toContain('data-testid="ai-novelai-execute"');
  });
});

describe("AiProviderPanel provider gates and accessibility", () => {
  it("keeps NovelAI exact contract values blank, labelled, warned, and gated", () => {
    const html = renderPanel("novelai");

    expect(html).toContain('for="ai-novelai-model"');
    expect(html).toContain('for="ai-novelai-action"');
    expect(html).toContain("기본값을 제공하지 않으며");
    expect(html).toContain("호환성을 보장하지 않습니다.");
    expect(html).toContain("Image Anlas");
    expect(html).toContain("사람이 지금 직접 시작하는 1회 요청");
    expect(openingTag(html, "ai-novelai-execute")).toContain('disabled=""');
    expect(html).toContain("이 이미지 1장 수정");
  });

  it("shows Gemini as a paid experimental private pilot with every eligibility gate", () => {
    const html = renderPanel("gemini");

    expect(html).toContain("실험실 · 비공개 파일럿");
    expect(html).toContain("일반 소비자용 무료 기능을 약속하지 않습니다.");
    expect(html).toContain("gemini-2.5-flash-image");
    expect(html).toContain("gemini-3.1-flash-image");
    expect(html).toContain("만 18세 이상");
    expect(html).toContain("미성년자를 대상으로 하거나");
    expect(html).toContain("전문적 또는 사업 목적");
    expect(html).toContain("지원 대상");
    expect(html).toContain("Paid Services");
    expect(openingTag(html, "ai-gemini-execute")).toContain('disabled=""');
  });

  it("shows one guided web handoff action without exposing internal manifest controls", () => {
    const html = renderPanel("web");
    const resourceValues = Array.from(
      html.matchAll(/data-resource="([^"]+)"/g),
      ([, value]) => value,
    );

    expect(resourceValues).toEqual([]);
    expect(html).toContain("전달 이미지와 구조 보호 프롬프트");
    expect(html).toContain("로그인과 실제");
    expect(html).toContain("업로드·생성·다운로드");
    expect(html).toContain("웹 AI로 바로 준비");
    expect(html).toContain('for="ai-web-handoff-request"');
    expect(html).not.toContain("Manifest");
  });

  it("uses native labels, fieldsets, button types, and described-by readiness", () => {
    const html = renderPanel("novelai");

    expect(html).toContain("<fieldset");
    expect(html).toContain("<legend");
    expect(html).toContain('aria-describedby="ai-novelai-readiness"');
    expect(html).toContain('aria-describedby="ai-novelai-credential-help"');
    expect(html).toContain('type="button"');
  });
});
