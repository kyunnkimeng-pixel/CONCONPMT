import { useEffect, useState } from "react";
import { Pause, Play, RotateCcw } from "lucide-react";

import type {
  MotionPreviewDto,
  MotionPreviewLoopMode,
  MotionTimingSource,
} from "@/features/editor/types";
import { usePrefersReducedMotion } from "@/lib/use-prefers-reduced-motion";
import { cn } from "@/lib/utils";

export function MotionPreviewPanel({
  hasActiveMotion,
  isAnimatedSource,
  isFresh,
  isMeasuring,
  measurement,
  prefersReducedMotionOverride,
}: {
  hasActiveMotion: boolean;
  isAnimatedSource: boolean;
  isFresh: boolean;
  isMeasuring: boolean;
  measurement: MotionPreviewDto | null;
  prefersReducedMotionOverride?: boolean;
}) {
  const systemPrefersReducedMotion = usePrefersReducedMotion();
  const prefersReducedMotion =
    prefersReducedMotionOverride ?? systemPrefersReducedMotion;
  const [isPlaying, setIsPlaying] = useState(false);
  const [playbackNonce, setPlaybackNonce] = useState(0);
  const outputIsAnimated = isAnimatedSource || hasActiveMotion;

  useEffect(() => {
    setIsPlaying(
      Boolean(
        measurement && isFresh && outputIsAnimated && !prefersReducedMotion,
      ),
    );
    setPlaybackNonce((current) => current + 1);
  }, [
    isFresh,
    measurement?.renderSignature,
    outputIsAnimated,
    prefersReducedMotion,
  ]);

  const canPlay = Boolean(
    outputIsAnimated &&
      measurement?.previewPath &&
      isFresh &&
      !prefersReducedMotion,
  );
  const showAnimatedPreview = canPlay && isPlaying;
  const previewSource = showAnimatedPreview
    ? withPlaybackNonce(measurement?.previewPath ?? "", playbackNonce)
    : measurement?.posterPath ||
      (!outputIsAnimated ? measurement?.previewPath ?? "" : "");

  return (
    <div className="flex min-h-0 flex-col gap-3">
      <section
        aria-labelledby="motion-preview-heading"
        className="rounded-md border border-border bg-white p-3"
        data-testid="motion-preview-panel"
      >
        <div className="mb-3 flex flex-wrap items-start justify-between gap-2">
          <div>
            <h4
              className="text-sm font-semibold tracking-normal"
              id="motion-preview-heading"
            >
              {outputIsAnimated
                ? "실제 GIF 모션 미리보기"
                : "정적 출력 미리보기"}
            </h4>
            <p className="mt-1 text-[11px] text-muted">
              {outputIsAnimated
                ? "native renderer로 GIF를 인코딩하고 조각별 용량을 측정합니다."
                : "활성 모션이 없어 정적 미리보기와 조각별 용량을 측정합니다."}
            </p>
          </div>
          <span
            className={cn(
              "rounded-full border px-2 py-0.5 text-[11px]",
              isMeasuring || (measurement && !isFresh)
                ? "border-blue-200 bg-blue-50 text-blue-800"
                : "border-border bg-canvas text-muted",
            )}
          >
            {isMeasuring
              ? outputIsAnimated
                ? "GIF 생성·측정 중"
                : "정적 미리보기 측정 중"
              : measurement
                ? isFresh
                  ? "최신 측정"
                  : "이전 측정"
                : "측정 필요"}
          </span>
        </div>
        <p className="mb-3 rounded-md border border-blue-200 bg-blue-50 px-3 py-2 text-[11px] text-blue-900">
          현재 편집 미리보기 기준 용량입니다. 최종 용량은 내보내기
          프로필·최적화를 적용한 뒤 내보내기 검증에서 다시 계산됩니다.
        </p>

        <div className="relative mx-auto flex aspect-square w-full max-w-96 items-center justify-center overflow-hidden rounded-md border border-border bg-preview p-3">
          {previewSource ? (
            <img
              alt={
                showAnimatedPreview
                  ? "실제 인코딩된 모션 GIF 미리보기"
                  : outputIsAnimated
                    ? "모션 GIF 정지 프레임"
                    : "정적 출력 미리보기"
              }
              className={cn(
                "max-h-full max-w-full object-contain",
                !isFresh && "opacity-60",
              )}
              draggable={false}
              src={previewSource}
            />
          ) : (
            <p className="px-4 text-center text-sm text-muted">
              {outputIsAnimated
                ? "설정을 고른 뒤 GIF 미리보기·용량 측정을 실행하세요."
                : "활성 모션이 없습니다. 정적 미리보기·용량 측정을 실행하세요."}
            </p>
          )}
          {isMeasuring ? (
            <div className="absolute inset-x-3 bottom-3 rounded-md bg-surface/95 px-3 py-2 text-center text-xs text-muted shadow">
              {outputIsAnimated
                ? "모든 프레임을 native renderer로 만드는 중입니다."
                : "정적 결과를 native renderer로 만드는 중입니다."}
            </div>
          ) : null}
        </div>

        <div className="mt-3 flex flex-wrap items-center justify-center gap-2">
          <button
            aria-label={isPlaying ? "모션 일시정지" : "모션 재생"}
            className={smallButtonClass}
            disabled={!canPlay}
            type="button"
            onClick={() => setIsPlaying((current) => !current)}
          >
            {isPlaying ? <Pause aria-hidden="true" /> : <Play aria-hidden="true" />}
            {isPlaying ? "일시정지" : "재생"}
          </button>
          <button
            className={smallButtonClass}
            disabled={!canPlay}
            type="button"
            onClick={() => {
              setPlaybackNonce((current) => current + 1);
              setIsPlaying(true);
            }}
          >
            <RotateCcw aria-hidden="true" />
            처음부터
          </button>
        </div>

        {prefersReducedMotion && outputIsAnimated ? (
          <p className="mt-2 text-center text-xs text-muted" role="status">
            시스템의 동작 줄이기 설정에 따라 애니메이션 재생을 사용하지 않고
            정지 프레임만 표시합니다.
          </p>
        ) : null}
        {!isAnimatedSource && hasActiveMotion ? (
          <p className="mt-2 rounded-md border border-blue-200 bg-blue-50 px-3 py-2 text-xs text-blue-900">
            정적 원본에 모션을 사용하면 이 아이콘은 GIF 출력으로 처리됩니다.
          </p>
        ) : null}
      </section>

      {measurement ? (
        <section
          aria-label={
            outputIsAnimated
              ? "모션 GIF 실제 측정 결과"
              : "정적 출력 실제 측정 결과"
          }
          className={cn(
            "rounded-md border border-border bg-canvas p-3",
            !isFresh && "opacity-65",
          )}
        >
          {!isFresh ? (
            <p className="mb-2 rounded bg-blue-50 px-2 py-1 text-xs text-blue-900">
              설정 또는 저장된 원본 상태가 바뀌었습니다. 아래 값은 이전
              측정이며 저장에 사용할 수 없습니다.
            </p>
          ) : null}
          <dl className="grid grid-cols-2 gap-2 text-xs">
            <PreviewStat
              danger={!measurement.passesByteLimit}
              label="가장 큰 미리보기 조각"
              value={`${formatBytes(measurement.maxPieceByteSize)} / ${formatBytes(
                measurement.maxBytes,
              )}`}
            />
            <PreviewStat
              label={outputIsAnimated ? "미리보기 GIF" : "미리보기 파일"}
              value={formatBytes(measurement.byteSize)}
            />
            {outputIsAnimated ? (
              <>
                <PreviewStat
                  label="프레임"
                  value={`${measurement.frameCount}개`}
                />
                <PreviewStat
                  label="길이"
                  value={formatDuration(measurement.durationMs)}
                />
                <PreviewStat
                  label="실제 FPS"
                  value={formatFps(measurement.effectiveFps)}
                />
                <PreviewStat
                  label="타이밍"
                  value={timingSourceLabel(measurement.timingSource)}
                />
                <PreviewStat
                  label="반복"
                  value={loopLabel(measurement.loopMode, measurement.loopCount)}
                />
              </>
            ) : (
              <PreviewStat
                label="현재 출력"
                value="활성 모션 없음 · 정적 출력"
              />
            )}
            <PreviewStat
              danger={measurement.clipped}
              label="가장자리 잘림"
              value={
                measurement.clipped
                  ? outputIsAnimated
                    ? `${measurement.clippedFrameCount}개 프레임 감지`
                    : "감지됨"
                  : "감지되지 않음"
              }
            />
            <PreviewStat
              label="처리 시간"
              value={`${measurement.processingMs}ms`}
            />
            <PreviewStat
              label="조각별 용량"
              value={
                measurement.pieceByteSizes.length
                  ? measurement.pieceByteSizes
                      .map((bytes, index) => `${index + 1}: ${formatBytes(bytes)}`)
                      .join(" · ")
                  : "-"
              }
            />
          </dl>
        </section>
      ) : null}

      {measurement?.warnings.length ? (
        <section
          aria-labelledby="motion-warning-heading"
          className="rounded-md border border-amber-300 bg-amber-50 p-3"
        >
          <h4
            className="text-xs font-semibold text-amber-900"
            id="motion-warning-heading"
          >
            확인할 점
          </h4>
          <ul className="mt-1 list-disc space-y-1 pl-4 text-xs text-amber-900">
            {measurement.warnings.map((warning, index) => (
              <li key={`${index}-${warning}`}>{warning}</li>
            ))}
          </ul>
        </section>
      ) : null}
    </div>
  );
}

function PreviewStat({
  danger = false,
  label,
  value,
}: {
  danger?: boolean;
  label: string;
  value: string;
}) {
  return (
    <div className="rounded-md border border-border bg-white px-3 py-2">
      <dt className="text-muted">{label}</dt>
      <dd className={cn("mt-0.5 font-semibold", danger && "text-danger")}>
        {value}
      </dd>
    </div>
  );
}

function withPlaybackNonce(path: string, nonce: number) {
  if (!path) {
    return "";
  }
  return `${path}${path.includes("?") ? "&" : "?"}motionPlayback=${nonce}`;
}

function timingSourceLabel(value: MotionTimingSource) {
  switch (value) {
    case "source_gif":
      return "원본 GIF 프레임 시간";
    case "generated":
      return "설정한 길이·FPS";
  }
}

function loopLabel(mode: MotionPreviewLoopMode, count: number | null) {
  switch (mode) {
    case "count":
      return `${count ?? 1}회`;
    case "once":
      return "한 번";
    case "infinite":
      return "무한 반복";
    case "pingpong":
      return "핑퐁 반복";
  }
}

function formatDuration(durationMs: number) {
  return durationMs < 1_000
    ? `${durationMs}ms`
    : `${(durationMs / 1_000).toFixed(durationMs % 1_000 === 0 ? 0 : 2)}초`;
}

function formatFps(fps: number) {
  return Number.isInteger(fps) ? `${fps}fps` : `${fps.toFixed(2)}fps 평균`;
}

function formatBytes(bytes: number) {
  if (bytes < 1024) {
    return `${bytes} B`;
  }
  if (bytes < 1024 * 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`;
  }
  return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
}

const smallButtonClass =
  "inline-flex items-center gap-1 rounded-md border border-border bg-white px-2.5 py-2 text-xs font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:text-muted";
