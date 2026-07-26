import { useEffect, useId, useState } from "react";
import type { ReactNode } from "react";

import {
  disableAllMotion,
  enabledMotionCount,
  hasEnabledMotion,
  MOTION_CATEGORY_OPTIONS,
  MOTION_LIMITS,
  MOTION_PRESET_OPTIONS,
  motionFrameCount,
  motionPresetLabel,
  motionPresetSummary,
  nextMotionSeed,
  resetMotionPreset,
  setMotionPreset,
  updateMotionPreset,
  type MotionCategory,
  type MotionPreset,
  type MotionPresetKind,
} from "@/features/editor/motion-recipe-model";
import type { MotionRecipeV1 } from "@/features/editor/types";
import { cn } from "@/lib/utils";

export interface MotionRecipeEditorProps {
  disabled?: boolean;
  isAnimatedSource: boolean;
  measuredDurationMs?: number | null;
  measuredFps?: number | null;
  recipe: MotionRecipeV1;
  sourceFrameCount: number | null;
  onChange: (recipe: MotionRecipeV1) => void;
}

export function MotionRecipeEditor({
  disabled = false,
  isAnimatedSource,
  measuredDurationMs = null,
  measuredFps = null,
  recipe,
  sourceFrameCount,
  onChange,
}: MotionRecipeEditorProps) {
  const headingId = useId();
  const enabledCount = enabledMotionCount(recipe);
  const hasActiveMotion = hasEnabledMotion(recipe);

  return (
    <section
      aria-labelledby={headingId}
      className="flex min-w-0 flex-col gap-3 rounded-md border border-border bg-canvas p-3"
      data-testid="motion-recipe-editor"
    >
      <div className="flex flex-wrap items-start justify-between gap-2">
        <div>
          <div className="flex items-center gap-2">
            <h4 className="text-sm font-semibold tracking-normal" id={headingId}>
              모션 효과
            </h4>
            <span className="rounded-full border border-border bg-white px-2 py-0.5 text-[11px] text-muted">
              {enabledCount}/4개 사용
            </span>
          </div>
          <p className="mt-1 text-[11px] leading-4 text-muted">
            공간 변형 → 일렁임·변위 → 색상·불투명도 → 오버레이 순서로
            결합 화면에 적용합니다.
          </p>
        </div>
        <button
          className={smallButtonClass}
          disabled={disabled || enabledCount === 0}
          title="범주와 매개변수는 유지하고 네 모션의 사용 상태만 끕니다."
          type="button"
          onClick={() => onChange(disableAllMotion(recipe))}
        >
          모든 모션 끄기
        </button>
      </div>

      <TimingPanel
        disabled={disabled}
        hasActiveMotion={hasActiveMotion}
        isAnimatedSource={isAnimatedSource}
        measuredDurationMs={measuredDurationMs}
        measuredFps={measuredFps}
        recipe={recipe}
        sourceFrameCount={sourceFrameCount}
        onChange={onChange}
      />
      <SamplingFields
        disabled={disabled || !hasActiveMotion}
        inactive={!hasActiveMotion}
        recipe={recipe}
        onChange={onChange}
      />

      <ol
        aria-label="모션 고정 합성 순서"
        className="flex min-w-0 flex-col gap-2"
      >
        {MOTION_CATEGORY_OPTIONS.map((option, index) => (
          <MotionCategoryCard
            category={option.category}
            description={option.description}
            disabled={disabled}
            index={index}
            key={option.category}
            label={option.label}
            preset={recipe[option.category]}
            recipe={recipe}
            onChange={onChange}
          />
        ))}
      </ol>
    </section>
  );
}

function TimingPanel({
  disabled,
  hasActiveMotion,
  isAnimatedSource,
  measuredDurationMs,
  measuredFps,
  recipe,
  sourceFrameCount,
  onChange,
}: {
  disabled: boolean;
  hasActiveMotion: boolean;
  isAnimatedSource: boolean;
  measuredDurationMs: number | null;
  measuredFps: number | null;
  recipe: MotionRecipeV1;
  sourceFrameCount: number | null;
  onChange: (recipe: MotionRecipeV1) => void;
}) {
  return (
    <section
      aria-label="모션 타이밍과 패턴"
      className="rounded-md border border-border bg-white p-3"
    >
      <div className="mb-3 flex flex-wrap items-start justify-between gap-2">
        <div>
          <h5 className="text-xs font-semibold">타이밍과 패턴</h5>
          <p className="mt-1 text-[11px] text-muted">
            {isAnimatedSource
              ? "원본 GIF의 실제 프레임 시간을 유지하고 누적 시간으로 모션 위상을 계산합니다."
              : hasActiveMotion
                ? "정적 이미지는 아래 길이와 FPS로 새 GIF가 됩니다."
                : "활성 모션 없음 · 현재 정적 출력 형식을 유지합니다."}
          </p>
        </div>
        <button
          className={smallButtonClass}
          disabled={disabled || !hasActiveMotion}
          title={
            hasActiveMotion
              ? "결정적으로 저장되는 패턴 seed만 바꿉니다."
              : "패턴을 바꾸려면 모션을 하나 이상 켜세요."
          }
          type="button"
          onClick={() =>
            onChange({ ...recipe, seed: nextMotionSeed(recipe.seed) })
          }
        >
          패턴 바꾸기
        </button>
      </div>

      {isAnimatedSource ? (
        <dl className="grid gap-2 text-xs sm:grid-cols-3">
          <ReadOnlyMetric
            label="원본 프레임"
            value={sourceFrameCount === null ? "측정 전" : `${sourceFrameCount}개`}
          />
          <ReadOnlyMetric
            label="실제 길이"
            value={
              measuredDurationMs === null
                ? "측정 후 표시"
                : formatDuration(measuredDurationMs)
            }
          />
          <ReadOnlyMetric
            label="실제 FPS"
            value={
              measuredFps === null ? "원본 타이밍" : `${formatFps(measuredFps)}`
            }
          />
        </dl>
      ) : hasActiveMotion ? (
        <div className="grid gap-3 sm:grid-cols-2">
          <RangeNumberField
            disabled={disabled}
            label="한 루프 길이"
            max={MOTION_LIMITS.durationMs.max}
            min={MOTION_LIMITS.durationMs.min}
            step={100}
            suffix="ms"
            value={recipe.durationMs}
            onChange={(durationMs) => onChange({ ...recipe, durationMs })}
          />
          <RangeNumberField
            disabled={disabled}
            label="GIF FPS"
            max={MOTION_LIMITS.fps.max}
            min={MOTION_LIMITS.fps.min}
            suffix="fps"
            value={recipe.fps}
            onChange={(fps) => onChange({ ...recipe, fps })}
          />
          <ReadOnlyMetric
            label="예상 프레임"
            value={`${motionFrameCount(recipe)}개`}
          />
          <ReadOnlyMetric label="출력 형식" value="GIF" />
        </div>
      ) : (
        <dl className="grid gap-2 text-xs">
          <ReadOnlyMetric
            label="현재 출력"
            value="활성 모션 없음 · 정적 출력"
          />
        </dl>
      )}

      <p className="mt-2 text-[11px] text-muted">
        패턴 번호 {recipe.seed} · 같은 설정과 패턴 번호는 같은 결과를 만듭니다.
      </p>
    </section>
  );
}

function MotionCategoryCard({
  category,
  description,
  disabled,
  index,
  label,
  preset,
  recipe,
  onChange,
}: {
  category: MotionCategory;
  description: string;
  disabled: boolean;
  index: number;
  label: string;
  preset: MotionPreset | null;
  recipe: MotionRecipeV1;
  onChange: (recipe: MotionRecipeV1) => void;
}) {
  const panelId = useId();
  const [expanded, setExpanded] = useState(preset !== null);

  useEffect(() => {
    if (preset !== null) {
      setExpanded(true);
    }
  }, [preset?.kind, preset !== null]);

  const options = MOTION_PRESET_OPTIONS[category];
  const changePreset = (kind: string) => {
    const nextKind = kind === "" ? null : (kind as MotionPresetKind);
    onChange(setMotionPreset(recipe, category, nextKind));
  };

  return (
    <li>
      <article
        aria-label={`${index + 1}번째 모션 범주 ${label}`}
        className={cn(
          "rounded-md border border-border bg-white",
          preset && !preset.enabled && "opacity-75",
        )}
      >
        <div className="flex min-w-0 items-center gap-2 p-2">
          <span
            aria-hidden="true"
            className="flex size-6 shrink-0 items-center justify-center rounded-full bg-preview text-[11px] font-semibold text-muted"
          >
            {index + 1}
          </span>
          <button
            aria-controls={panelId}
            aria-expanded={expanded}
            className="min-w-0 flex-1 rounded px-1 py-1 text-left hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
            type="button"
            onClick={() => setExpanded((current) => !current)}
          >
            <span className="block truncate text-xs font-semibold">{label}</span>
            <span className="block truncate text-[11px] text-muted">
              {!preset
                ? "사용 안 함"
                : preset.enabled
                  ? `${motionPresetLabel(preset.kind)} · ${motionPresetSummary(preset)}`
                  : `${motionPresetLabel(preset.kind)} · 사용 안 함`}
            </span>
          </button>
          {preset ? (
            <label className="flex shrink-0 items-center gap-1 text-[11px] text-muted">
              <input
                aria-label={`${label} 사용`}
                checked={preset.enabled}
                disabled={disabled}
                type="checkbox"
                onChange={(event) =>
                  onChange(
                    updateMotionPreset(recipe, category, (current) => ({
                      ...current,
                      enabled: event.currentTarget.checked,
                    })),
                  )
                }
              />
              사용
            </label>
          ) : null}
        </div>

        {expanded ? (
          <div className="border-t border-border p-3" id={panelId}>
            <label className="flex flex-col gap-1 text-xs font-medium text-muted">
              {label} 프리셋
              <select
                className={inputClass}
                disabled={disabled}
                value={preset?.kind ?? ""}
                onChange={(event) => changePreset(event.currentTarget.value)}
              >
                <option value="">사용 안 함</option>
                {options.map((option) => (
                  <option key={option.kind} value={option.kind}>
                    {option.label}
                  </option>
                ))}
              </select>
            </label>
            <p className="mt-1 text-[11px] text-muted">{description}</p>

            {preset ? (
              <>
                <div className="mt-3">
                  <MotionPresetFields
                    category={category}
                    disabled={disabled || !preset.enabled}
                    preset={preset}
                    recipe={recipe}
                    onChange={onChange}
                  />
                </div>
                <div className="mt-3 flex justify-end">
                  <button
                    className={smallButtonClass}
                    disabled={disabled}
                    title="이 범주의 프리셋 매개변수만 권장값으로 되돌립니다."
                    type="button"
                    onClick={() =>
                      onChange(resetMotionPreset(recipe, category))
                    }
                  >
                    이 범주 기본값
                  </button>
                </div>
              </>
            ) : null}
          </div>
        ) : null}
      </article>
    </li>
  );
}

function MotionPresetFields({
  category,
  disabled,
  preset,
  recipe,
  onChange,
}: {
  category: MotionCategory;
  disabled: boolean;
  preset: MotionPreset;
  recipe: MotionRecipeV1;
  onChange: (recipe: MotionRecipeV1) => void;
}) {
  const update = (nextPreset: MotionPreset) =>
    onChange(updateMotionPreset(recipe, category, () => nextPreset));
  const cycles = (
    <RangeNumberField
      disabled={disabled}
      label="루프당 횟수"
      max={MOTION_LIMITS.cyclesPerLoop.max}
      min={MOTION_LIMITS.cyclesPerLoop.min}
      suffix="회"
      value={preset.cyclesPerLoop}
      onChange={(cyclesPerLoop) => update({ ...preset, cyclesPerLoop })}
    />
  );

  switch (preset.kind) {
    case "shake":
      return (
        <FieldGrid>
          <RangeNumberField
            disabled={disabled}
            label="가로 진폭"
            max={MOTION_LIMITS.amplitude.max}
            min={MOTION_LIMITS.amplitude.min}
            suffix="px"
            value={preset.amplitudeX}
            onChange={(amplitudeX) => update({ ...preset, amplitudeX })}
          />
          <RangeNumberField
            disabled={disabled}
            label="세로 진폭"
            max={MOTION_LIMITS.amplitude.max}
            min={MOTION_LIMITS.amplitude.min}
            suffix="px"
            value={preset.amplitudeY}
            onChange={(amplitudeY) => update({ ...preset, amplitudeY })}
          />
          {cycles}
        </FieldGrid>
      );
    case "bounce":
      return (
        <FieldGrid>
          <RangeNumberField
            disabled={disabled}
            label="튀는 높이"
            max={MOTION_LIMITS.amplitude.max}
            min={MOTION_LIMITS.amplitude.min}
            suffix="px"
            value={preset.heightPx}
            onChange={(heightPx) => update({ ...preset, heightPx })}
          />
          {cycles}
        </FieldGrid>
      );
    case "breathe":
      return (
        <FieldGrid>
          <RangeNumberField
            disabled={disabled}
            label="크기 변화"
            max={MOTION_LIMITS.scalePercent.max}
            min={MOTION_LIMITS.scalePercent.min}
            suffix="%"
            value={preset.scalePercent}
            onChange={(scalePercent) => update({ ...preset, scalePercent })}
          />
          {cycles}
        </FieldGrid>
      );
    case "rock":
      return (
        <FieldGrid>
          <RangeNumberField
            disabled={disabled}
            label="회전 각도"
            max={MOTION_LIMITS.angleDegrees.max}
            min={MOTION_LIMITS.angleDegrees.min}
            suffix="°"
            value={preset.angleDegrees}
            onChange={(angleDegrees) => update({ ...preset, angleDegrees })}
          />
          {cycles}
        </FieldGrid>
      );
    case "spin":
      return (
        <FieldGrid>
          <SelectField
            disabled={disabled}
            label="회전 방향"
            value={preset.clockwise ? "clockwise" : "counter_clockwise"}
            options={[
              { value: "clockwise", label: "시계 방향" },
              { value: "counter_clockwise", label: "반시계 방향" },
            ]}
            onChange={(value) =>
              update({ ...preset, clockwise: value === "clockwise" })
            }
          />
          {cycles}
        </FieldGrid>
      );
    case "wave":
      return (
        <>
          <FieldGrid>
            <SelectField
              disabled={disabled}
              label="물결 방향"
              value={preset.axis}
              options={[
                { value: "horizontal", label: "가로" },
                { value: "vertical", label: "세로" },
              ]}
              onChange={(axis) =>
                update({
                  ...preset,
                  axis: axis === "vertical" ? "vertical" : "horizontal",
                })
              }
            />
            <RangeNumberField
              disabled={disabled}
              label="진폭"
              max={MOTION_LIMITS.amplitude.max}
              min={MOTION_LIMITS.amplitude.min}
              suffix="px"
              value={preset.amplitudePx}
              onChange={(amplitudePx) => update({ ...preset, amplitudePx })}
            />
            <RangeNumberField
              disabled={disabled}
              label="파장"
              max={MOTION_LIMITS.wavelength.max}
              min={MOTION_LIMITS.wavelength.min}
              suffix="px"
              value={preset.wavelengthPx}
              onChange={(wavelengthPx) => update({ ...preset, wavelengthPx })}
            />
            {cycles}
          </FieldGrid>
        </>
      );
    case "jelly":
      return (
        <>
          <FieldGrid>
            <RangeNumberField
              disabled={disabled}
              label="가로 진폭"
              max={MOTION_LIMITS.amplitude.max}
              min={MOTION_LIMITS.amplitude.min}
              suffix="px"
              value={preset.amplitudeX}
              onChange={(amplitudeX) => update({ ...preset, amplitudeX })}
            />
            <RangeNumberField
              disabled={disabled}
              label="세로 진폭"
              max={MOTION_LIMITS.amplitude.max}
              min={MOTION_LIMITS.amplitude.min}
              suffix="px"
              value={preset.amplitudeY}
              onChange={(amplitudeY) => update({ ...preset, amplitudeY })}
            />
            <RangeNumberField
              disabled={disabled}
              label="가로 파장"
              max={MOTION_LIMITS.wavelength.max}
              min={MOTION_LIMITS.wavelength.min}
              suffix="px"
              value={preset.wavelengthX}
              onChange={(wavelengthX) => update({ ...preset, wavelengthX })}
            />
            <RangeNumberField
              disabled={disabled}
              label="세로 파장"
              max={MOTION_LIMITS.wavelength.max}
              min={MOTION_LIMITS.wavelength.min}
              suffix="px"
              value={preset.wavelengthY}
              onChange={(wavelengthY) => update({ ...preset, wavelengthY })}
            />
            {cycles}
          </FieldGrid>
        </>
      );
    case "ripple":
      return (
        <>
          <FieldGrid>
            <RangeNumberField
              disabled={disabled}
              label="진폭"
              max={MOTION_LIMITS.amplitude.max}
              min={MOTION_LIMITS.amplitude.min}
              suffix="px"
              value={preset.amplitudePx}
              onChange={(amplitudePx) => update({ ...preset, amplitudePx })}
            />
            <RangeNumberField
              disabled={disabled}
              label="파장"
              max={MOTION_LIMITS.wavelength.max}
              min={MOTION_LIMITS.wavelength.min}
              suffix="px"
              value={preset.wavelengthPx}
              onChange={(wavelengthPx) => update({ ...preset, wavelengthPx })}
            />
            <RangeNumberField
              disabled={disabled}
              label="중심 X"
              max={100}
              min={0}
              suffix="%"
              value={preset.centerXPercent}
              onChange={(centerXPercent) =>
                update({ ...preset, centerXPercent })
              }
            />
            <RangeNumberField
              disabled={disabled}
              label="중심 Y"
              max={100}
              min={0}
              suffix="%"
              value={preset.centerYPercent}
              onChange={(centerYPercent) =>
                update({ ...preset, centerYPercent })
              }
            />
            {cycles}
          </FieldGrid>
        </>
      );
    case "glitchBands":
      return (
        <>
          <FieldGrid>
            <RangeNumberField
              disabled={disabled}
              label="가로 이동"
              max={MOTION_LIMITS.amplitude.max}
              min={MOTION_LIMITS.amplitude.min}
              suffix="px"
              value={preset.amplitudePx}
              onChange={(amplitudePx) => update({ ...preset, amplitudePx })}
            />
            <RangeNumberField
              disabled={disabled}
              label="밴드 높이"
              max={MOTION_LIMITS.bandHeight.max}
              min={MOTION_LIMITS.bandHeight.min}
              suffix="px"
              value={preset.bandHeightPx}
              onChange={(bandHeightPx) => update({ ...preset, bandHeightPx })}
            />
            <RangeNumberField
              disabled={disabled}
              label="단계 수"
              max={MOTION_LIMITS.stepsPerCycle.max}
              min={MOTION_LIMITS.stepsPerCycle.min}
              value={preset.stepsPerCycle}
              onChange={(stepsPerCycle) =>
                update({ ...preset, stepsPerCycle })
              }
            />
            {cycles}
          </FieldGrid>
        </>
      );
    case "hueCycle":
      return (
        <FieldGrid>
          <RangeNumberField
            disabled={disabled}
            label="색조 범위"
            max={MOTION_LIMITS.hueRange.max}
            min={MOTION_LIMITS.hueRange.min}
            suffix="°"
            value={preset.rangeDegrees}
            onChange={(rangeDegrees) => update({ ...preset, rangeDegrees })}
          />
          {cycles}
        </FieldGrid>
      );
    case "tintPulse":
      return (
        <FieldGrid>
          <ColorField
            color={preset.color}
            disabled={disabled}
            label="지정색"
            onChange={(color) => update({ ...preset, color })}
          />
          <RangeNumberField
            disabled={disabled}
            label="최대 혼합"
            max={100}
            min={0}
            suffix="%"
            value={preset.amountPercent}
            onChange={(amountPercent) => update({ ...preset, amountPercent })}
          />
          {cycles}
        </FieldGrid>
      );
    case "brightnessSaturationPulse":
      return (
        <FieldGrid>
          <RangeNumberField
            disabled={disabled}
            label="밝기 변화"
            max={100}
            min={0}
            suffix="%"
            value={preset.brightnessPercent}
            onChange={(brightnessPercent) =>
              update({ ...preset, brightnessPercent })
            }
          />
          <RangeNumberField
            disabled={disabled}
            label="채도 변화"
            max={100}
            min={0}
            suffix="%"
            value={preset.saturationPercent}
            onChange={(saturationPercent) =>
              update({ ...preset, saturationPercent })
            }
          />
          {cycles}
        </FieldGrid>
      );
    case "flash":
      return (
        <FieldGrid>
          <ColorField
            color={preset.color}
            disabled={disabled}
            label="번쩍임 색"
            onChange={(color) => update({ ...preset, color })}
          />
          <RangeNumberField
            disabled={disabled}
            label="강도"
            max={100}
            min={0}
            suffix="%"
            value={preset.intensityPercent}
            onChange={(intensityPercent) =>
              update({ ...preset, intensityPercent })
            }
          />
          {cycles}
        </FieldGrid>
      );
    case "focusLines":
      return (
        <FieldGrid>
          <ColorField
            color={preset.color}
            disabled={disabled}
            label="선 색"
            onChange={(color) => update({ ...preset, color })}
          />
          <RangeNumberField
            disabled={disabled}
            label="선 개수"
            max={MOTION_LIMITS.lineCount.max}
            min={MOTION_LIMITS.lineCount.min}
            value={preset.lineCount}
            onChange={(lineCount) => update({ ...preset, lineCount })}
          />
          <RangeNumberField
            disabled={disabled}
            label="선 두께"
            max={MOTION_LIMITS.lineWidth.max}
            min={MOTION_LIMITS.lineWidth.min}
            suffix="px"
            value={preset.lineWidthPx}
            onChange={(lineWidthPx) => update({ ...preset, lineWidthPx })}
          />
          <RangeNumberField
            disabled={disabled}
            label="안쪽 반경"
            max={MOTION_LIMITS.innerRadiusPercent.max}
            min={MOTION_LIMITS.innerRadiusPercent.min}
            suffix="%"
            value={preset.innerRadiusPercent}
            onChange={(innerRadiusPercent) =>
              update({ ...preset, innerRadiusPercent })
            }
          />
          <RangeNumberField
            disabled={disabled}
            label="불투명도"
            max={100}
            min={0}
            suffix="%"
            value={preset.opacityPercent}
            onChange={(opacityPercent) =>
              update({ ...preset, opacityPercent })
            }
          />
          {cycles}
        </FieldGrid>
      );
    case "sparkle":
      return (
        <FieldGrid>
          <ColorField
            color={preset.color}
            disabled={disabled}
            label="반짝이 색"
            onChange={(color) => update({ ...preset, color })}
          />
          <RangeNumberField
            disabled={disabled}
            label="개수"
            max={MOTION_LIMITS.sparkleCount.max}
            min={MOTION_LIMITS.sparkleCount.min}
            value={preset.count}
            onChange={(count) => update({ ...preset, count })}
          />
          <RangeNumberField
            disabled={disabled}
            label="크기"
            max={MOTION_LIMITS.sparkleSize.max}
            min={MOTION_LIMITS.sparkleSize.min}
            suffix="px"
            value={preset.sizePx}
            onChange={(sizePx) => update({ ...preset, sizePx })}
          />
          <RangeNumberField
            disabled={disabled}
            label="불투명도"
            max={100}
            min={0}
            suffix="%"
            value={preset.opacityPercent}
            onChange={(opacityPercent) =>
              update({ ...preset, opacityPercent })
            }
          />
          {cycles}
        </FieldGrid>
      );
    case "expansionRing":
      return (
        <FieldGrid>
          <ColorField
            color={preset.color}
            disabled={disabled}
            label="링 색"
            onChange={(color) => update({ ...preset, color })}
          />
          <RangeNumberField
            disabled={disabled}
            label="선 두께"
            max={MOTION_LIMITS.lineWidth.max}
            min={MOTION_LIMITS.lineWidth.min}
            suffix="px"
            value={preset.lineWidthPx}
            onChange={(lineWidthPx) => update({ ...preset, lineWidthPx })}
          />
          <RangeNumberField
            disabled={disabled}
            label="최대 반경"
            max={MOTION_LIMITS.ringRadius.max}
            min={MOTION_LIMITS.ringRadius.min}
            suffix="%"
            value={preset.maxRadiusPercent}
            onChange={(maxRadiusPercent) =>
              update({ ...preset, maxRadiusPercent })
            }
          />
          <RangeNumberField
            disabled={disabled}
            label="불투명도"
            max={100}
            min={0}
            suffix="%"
            value={preset.opacityPercent}
            onChange={(opacityPercent) =>
              update({ ...preset, opacityPercent })
            }
          />
          {cycles}
        </FieldGrid>
      );
  }
}

function SamplingFields({
  disabled,
  inactive,
  recipe,
  onChange,
}: {
  disabled: boolean;
  inactive: boolean;
  recipe: MotionRecipeV1;
  onChange: (recipe: MotionRecipeV1) => void;
}) {
  return (
    <section
      aria-label="모션 샘플링과 가장자리"
      className="rounded-md border border-border bg-white p-3"
    >
      <h5 className="text-xs font-semibold">샘플링과 가장자리</h5>
      <p className="mt-1 text-[11px] text-muted">
        {inactive
          ? "모션을 하나 이상 켜면 보간 방식과 가장자리 처리를 적용할 수 있습니다."
          : "픽셀을 옮기거나 회전할 때의 선명도와 화면 밖 픽셀 처리 방식을 정합니다."}
      </p>
      <div className="mt-3 grid gap-3 sm:grid-cols-2">
        <SelectField
          disabled={disabled}
          label="보간 방식"
          value={recipe.interpolation}
          options={[
            { value: "nearest", label: "Nearest · 픽셀아트" },
            { value: "bilinear", label: "Bilinear · 부드럽게" },
          ]}
          onChange={(interpolation) =>
            onChange({
              ...recipe,
              interpolation:
                interpolation === "nearest" ? "nearest" : "bilinear",
            })
          }
        />
        <SelectField
          disabled={disabled}
          label="가장자리 처리"
          value={recipe.edgeMode}
          options={[
            { value: "transparent", label: "투명" },
            { value: "clamp", label: "가장자리 고정" },
            { value: "mirror", label: "거울 반사" },
          ]}
          onChange={(edgeMode) =>
            onChange({
              ...recipe,
              edgeMode:
                edgeMode === "clamp" || edgeMode === "mirror"
                  ? edgeMode
                  : "transparent",
            })
          }
        />
      </div>
    </section>
  );
}

function FieldGrid({ children }: { children: ReactNode }) {
  return <div className="grid gap-3 sm:grid-cols-2">{children}</div>;
}

function RangeNumberField({
  disabled,
  label,
  max,
  min,
  step = 1,
  suffix,
  value,
  onChange,
}: {
  disabled: boolean;
  label: string;
  max: number;
  min: number;
  step?: number;
  suffix?: string;
  value: number;
  onChange: (value: number) => void;
}) {
  const labelId = useId();
  const normalizedValue = Math.min(max, Math.max(min, value));
  return (
    <div
      aria-labelledby={labelId}
      className="flex min-w-0 flex-col gap-1"
      role="group"
    >
      <span className="text-xs font-medium text-muted" id={labelId}>
        {label}
      </span>
      <div className="grid grid-cols-[minmax(70px,1fr)_72px_auto] items-center gap-2">
        <input
          aria-label={`${label} 슬라이더`}
          disabled={disabled}
          max={max}
          min={min}
          step={step}
          type="range"
          value={normalizedValue}
          onChange={(event) => onChange(Number(event.currentTarget.value))}
        />
        <input
          aria-label={`${label} 숫자 입력`}
          className={numberInputClass}
          disabled={disabled}
          max={max}
          min={min}
          step={step}
          type="number"
          value={normalizedValue}
          onChange={(event) => {
            const parsed = Number(event.currentTarget.value);
            if (Number.isFinite(parsed)) {
              onChange(parsed);
            }
          }}
        />
        <span aria-hidden="true" className="text-[11px] text-muted">
          {suffix ?? ""}
        </span>
      </div>
    </div>
  );
}

function SelectField({
  disabled,
  label,
  options,
  value,
  onChange,
}: {
  disabled: boolean;
  label: string;
  options: Array<{ value: string; label: string }>;
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <label className="flex flex-col gap-1 text-xs font-medium text-muted">
      {label}
      <select
        className={inputClass}
        disabled={disabled}
        value={value}
        onChange={(event) => onChange(event.currentTarget.value)}
      >
        {options.map((option) => (
          <option key={option.value} value={option.value}>
            {option.label}
          </option>
        ))}
      </select>
    </label>
  );
}

function ColorField({
  color,
  disabled,
  label,
  onChange,
}: {
  color: string;
  disabled: boolean;
  label: string;
  onChange: (color: string) => void;
}) {
  const [draftColor, setDraftColor] = useState(color);
  useEffect(() => setDraftColor(color), [color]);
  const valid = /^#[0-9a-f]{6}(?:[0-9a-f]{2})?$/i.test(draftColor.trim());
  const pickerColor = /^#[0-9a-f]{6}/i.test(color)
    ? color.slice(0, 7)
    : "#ffffff";
  const commit = () => {
    if (!valid) {
      setDraftColor(color);
      return;
    }
    const normalized = draftColor.trim().toLowerCase();
    setDraftColor(normalized);
    onChange(normalized);
  };

  return (
    <label className="flex flex-col gap-1 text-xs font-medium text-muted">
      {label}
      <span className="grid grid-cols-[44px_minmax(0,1fr)] gap-2">
        <input
          aria-label={`${label} 선택`}
          className="h-9 w-11 rounded-md border border-border bg-white p-1"
          disabled={disabled}
          type="color"
          value={pickerColor}
          onChange={(event) => {
            setDraftColor(event.currentTarget.value);
            onChange(event.currentTarget.value);
          }}
        />
        <input
          aria-label={`${label} 값`}
          aria-invalid={!valid}
          className={inputClass}
          disabled={disabled}
          maxLength={9}
          value={draftColor}
          onBlur={commit}
          onChange={(event) => setDraftColor(event.currentTarget.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              event.preventDefault();
              commit();
            } else if (event.key === "Escape") {
              setDraftColor(color);
            }
          }}
        />
      </span>
      {!valid ? (
        <span className="text-[11px] font-normal text-danger">
          #RRGGBB 또는 #RRGGBBAA 형식으로 입력하세요.
        </span>
      ) : null}
    </label>
  );
}

function ReadOnlyMetric({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-md border border-border bg-canvas px-3 py-2">
      <dt className="text-[11px] text-muted">{label}</dt>
      <dd className="mt-0.5 font-semibold">{value}</dd>
    </div>
  );
}

function formatDuration(durationMs: number) {
  return durationMs < 1_000
    ? `${durationMs}ms`
    : `${(durationMs / 1_000).toFixed(durationMs % 1_000 === 0 ? 0 : 2)}초`;
}

function formatFps(fps: number) {
  return Number.isInteger(fps) ? `${fps}fps` : `${fps.toFixed(2)}fps 평균`;
}

const inputClass =
  "min-w-0 rounded-md border border-border bg-white px-2 py-2 text-sm text-foreground focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:text-muted";
const numberInputClass =
  "min-w-0 rounded-md border border-border bg-white px-2 py-1.5 text-sm text-foreground focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:text-muted";
const smallButtonClass =
  "rounded-md border border-border bg-white px-2.5 py-1.5 text-xs font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:text-muted";
