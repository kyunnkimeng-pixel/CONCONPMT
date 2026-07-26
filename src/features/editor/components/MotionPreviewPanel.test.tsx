import { renderToString } from "react-dom/server";
import { describe, expect, it } from "vitest";

import { MotionPreviewPanel } from "@/features/editor/components/MotionPreviewPanel";
import type { MotionPreviewDto } from "@/features/editor/types";

const measurement: MotionPreviewDto = {
  previewPath: "asset://motion-preview.gif",
  posterPath: "asset://motion-poster.png",
  byteSize: 196_608,
  pieceByteSizes: [120_000, 140_000],
  maxPieceByteSize: 140_000,
  maxBytes: 2_097_152,
  passesByteLimit: true,
  frameCount: 15,
  durationMs: 1_250,
  effectiveFps: 12,
  timingSource: "generated",
  loopMode: "infinite",
  loopCount: null,
  clipped: true,
  clippedFrameCount: 2,
  processingMs: 87,
  warnings: ["큰 흔들림은 가장자리를 자를 수 있습니다."],
  renderSignature: "motion-render-signature",
  generatedAt: "2026-07-26T12:00:00Z",
};

describe("MotionPreviewPanel", () => {
  it("mounts only the poster when reduced motion is preferred", () => {
    const html = renderPanel({
      isFresh: true,
      measurement,
      prefersReducedMotionOverride: true,
    });
    const text = plainText(html);

    expect(html).toContain("asset://motion-poster.png");
    expect(html).not.toContain("asset://motion-preview.gif");
    expect(text).toContain(
      "시스템의 동작 줄이기 설정에 따라 애니메이션 재생을 사용하지 않고",
    );
    expect((html.match(/disabled=""/g) ?? []).length).toBeGreaterThanOrEqual(2);
    expect(text).toContain("최신 측정");
  });

  it("shows actual encoded metrics, piece sizes, clipping and warnings", () => {
    const html = renderPanel({
      isFresh: true,
      measurement,
      prefersReducedMotionOverride: true,
    });
    const text = plainText(html);

    expect(html).toContain('aria-label="모션 GIF 실제 측정 결과"');
    expect(text).toContain("136.7 KB / 2.00 MB");
    expect(text).toContain("192.0 KB");
    expect(text).toContain("15개");
    expect(text).toContain("1.25초");
    expect(text).toContain("12fps");
    expect(text).toContain("설정한 길이·FPS");
    expect(text).toContain("무한 반복");
    expect(text).toContain("2개 프레임 감지");
    expect(text).toContain("1: 117.2 KB · 2: 136.7 KB");
    expect(text).toContain("큰 흔들림은 가장자리를 자를 수 있습니다.");
    expect(text).toContain(
      "현재 편집 미리보기 기준 용량입니다. 최종 용량은 내보내기",
    );
    expect(text).toContain("내보내기 검증에서 다시 계산됩니다.");
  });

  it("labels the backend source_gif timing value as original GIF timing", () => {
    const html = renderPanel({
      isFresh: true,
      measurement: {
        ...measurement,
        timingSource: "source_gif",
      },
      prefersReducedMotionOverride: true,
    });
    const text = plainText(html);

    expect(text).toContain("원본 GIF 프레임 시간");
    expect(text).not.toContain("source_gif");
  });

  it.each([
    ["once", null, "한 번"],
    ["infinite", null, "무한 반복"],
    ["count", 3, "3회"],
    ["pingpong", null, "핑퐁 반복"],
  ] as const)(
    "labels the backend effective loop mode %s",
    (loopMode, loopCount, expectedLabel) => {
      const html = renderPanel({
        isFresh: true,
        measurement: {
          ...measurement,
          loopMode,
          loopCount,
        },
        prefersReducedMotionOverride: true,
      });

      expect(plainText(html)).toContain(expectedLabel);
    },
  );

  it("marks old measurements stale and explains static-source GIF output", () => {
    const html = renderPanel({
      isAnimatedSource: false,
      isFresh: false,
      measurement,
      prefersReducedMotionOverride: false,
    });
    const text = plainText(html);

    expect(text).toContain("이전 측정");
    expect(text).toContain(
      "설정 또는 저장된 원본 상태가 바뀌었습니다. 아래 값은 이전 측정이며 저장에 사용할 수 없습니다.",
    );
    expect(text).toContain(
      "정적 원본에 모션을 사용하면 이 아이콘은 GIF 출력으로 처리됩니다.",
    );
    expect(html).toContain("asset://motion-poster.png");
    expect(html).not.toContain("asset://motion-preview.gif");
  });

  it("shows static output without GIF timing when a static source has no active motion", () => {
    const html = renderPanel({
      hasActiveMotion: false,
      isAnimatedSource: false,
      isFresh: true,
      measurement,
      prefersReducedMotionOverride: false,
    });
    const text = plainText(html);

    expect(html).toContain('aria-label="정적 출력 실제 측정 결과"');
    expect(text).toContain("정적 출력 미리보기");
    expect(text).toContain("활성 모션 없음 · 정적 출력");
    expect(text).toContain("미리보기 파일");
    expect(text).not.toContain("실제 GIF 모션 미리보기");
    expect(text).not.toContain("GIF 출력으로 처리됩니다.");
    expect(text).not.toContain("프레임");
    expect(html).toContain("asset://motion-poster.png");
    expect(html).not.toContain("asset://motion-preview.gif");
    expect((html.match(/disabled=""/g) ?? []).length).toBeGreaterThanOrEqual(2);
  });

  it("does not mount an animated image before an explicit measurement exists", () => {
    const html = renderPanel({
      isFresh: false,
      measurement: null,
      prefersReducedMotionOverride: false,
    });
    const text = plainText(html);

    expect(text).toContain(
      "설정을 고른 뒤 GIF 미리보기·용량 측정을 실행하세요.",
    );
    expect(text).toContain("측정 필요");
    expect(html).not.toContain("<img");
  });
});

function renderPanel(
  overrides: Partial<{
    hasActiveMotion: boolean;
    isAnimatedSource: boolean;
    isFresh: boolean;
    isMeasuring: boolean;
    measurement: MotionPreviewDto | null;
    prefersReducedMotionOverride: boolean;
  }>,
) {
  return renderToString(
    <MotionPreviewPanel
      hasActiveMotion={overrides.hasActiveMotion ?? true}
      isAnimatedSource={overrides.isAnimatedSource ?? true}
      isFresh={overrides.isFresh ?? false}
      isMeasuring={overrides.isMeasuring ?? false}
      measurement={overrides.measurement ?? null}
      prefersReducedMotionOverride={overrides.prefersReducedMotionOverride}
    />,
  );
}

function plainText(html: string) {
  return html.replace(/<!-- -->/g, "");
}
