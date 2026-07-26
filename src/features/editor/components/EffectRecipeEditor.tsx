import { useEffect, useId, useState } from "react";

import {
  addEffect,
  disableAllEffects,
  EFFECT_KIND_OPTIONS,
  EFFECT_LIMITS,
  effectKindLabel,
  effectSummary,
  MAX_EFFECT_STEPS,
  moveEffectByOffset,
  removeEffect,
  resetEffect,
  toggleEffect,
  updateEffect,
  type EffectKind,
} from "@/features/editor/effect-recipe-model";
import type {
  EffectRecipeV1,
  IconEffect,
} from "@/features/editor/types";
import { cn } from "@/lib/utils";

interface EffectRecipeEditorProps {
  disabled?: boolean;
  recipe: EffectRecipeV1;
  onChange: (recipe: EffectRecipeV1) => void;
}

export function EffectRecipeEditor({
  disabled = false,
  recipe,
  onChange,
}: EffectRecipeEditorProps) {
  const headingId = useId();
  const addSelectId = useId();
  const [newEffectKind, setNewEffectKind] = useState<EffectKind>("pixelate");
  const [expandedEffectId, setExpandedEffectId] = useState<string | null>(
    recipe.effects[0]?.id ?? null,
  );
  const enabledCount = recipe.effects.filter((effect) => effect.enabled).length;
  const isAtLimit = recipe.effects.length >= MAX_EFFECT_STEPS;

  useEffect(() => {
    if (
      expandedEffectId !== null &&
      !recipe.effects.some((effect) => effect.id === expandedEffectId)
    ) {
      setExpandedEffectId(recipe.effects[0]?.id ?? null);
    }
  }, [expandedEffectId, recipe.effects]);

  const addSelectedEffect = () => {
    const nextRecipe = addEffect(recipe, newEffectKind);
    const addedEffect = nextRecipe.effects[nextRecipe.effects.length - 1] ?? null;
    onChange(nextRecipe);
    setExpandedEffectId(addedEffect?.id ?? null);
  };

  return (
    <section
      aria-labelledby={headingId}
      className="flex min-w-0 flex-col gap-3 rounded-md border border-border bg-canvas p-3"
      data-testid="effect-recipe-editor"
    >
      <div className="flex flex-wrap items-start justify-between gap-2">
        <div>
          <div className="flex items-center gap-2">
            <h4 className="text-sm font-semibold tracking-normal" id={headingId}>
              효과
            </h4>
            <span className="rounded-full border border-border bg-white px-2 py-0.5 text-[11px] text-muted">
              {enabledCount}개 사용
            </span>
          </div>
          <p className="mt-1 text-[11px] leading-4 text-muted">
            위에서 아래 순서로 결합 화면 전체에 적용한 뒤 조각을 나눕니다.
          </p>
        </div>
        <button
          className={smallButtonClass}
          disabled={disabled || enabledCount === 0}
          title="효과 항목과 매개변수는 유지하고 사용 상태만 끕니다."
          type="button"
          onClick={() => onChange(disableAllEffects(recipe))}
        >
          모든 효과 끄기
        </button>
      </div>

      <div
        aria-label="효과 추가"
        className="grid grid-cols-[minmax(0,1fr)_auto] items-end gap-2"
        role="group"
      >
        <label
          className="flex min-w-0 flex-col gap-1 text-xs font-medium text-muted"
          htmlFor={addSelectId}
        >
          효과 종류
          <select
            className={inputClass}
            disabled={disabled || isAtLimit}
            id={addSelectId}
            value={newEffectKind}
            onChange={(event) =>
              setNewEffectKind(event.currentTarget.value as EffectKind)
            }
          >
            {EFFECT_KIND_OPTIONS.map((option) => (
              <option key={option.kind} value={option.kind}>
                {option.label}
              </option>
            ))}
          </select>
        </label>
        <button
          className="rounded-md bg-accent px-3 py-2 text-sm font-semibold text-accent-foreground hover:bg-accent-strong focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:opacity-60"
          disabled={disabled || isAtLimit}
          type="button"
          onClick={addSelectedEffect}
        >
          효과 추가
        </button>
      </div>
      <p className="text-[11px] text-muted">
        {recipe.effects.length}/{MAX_EFFECT_STEPS}단계
        {isAtLimit ? " · 최대 단계에 도달했습니다." : ""}
      </p>

      {recipe.effects.length === 0 ? (
        <p className="rounded-md border border-dashed border-border bg-white px-3 py-5 text-center text-xs text-muted">
          적용할 효과가 없습니다. 효과 종류를 고른 뒤 추가하세요.
        </p>
      ) : (
        <ol
          aria-label="효과 적용 순서"
          className="flex min-w-0 flex-col gap-2"
        >
          {recipe.effects.map((effect, index) => (
            <EffectCard
              disabled={disabled}
              effect={effect}
              expanded={expandedEffectId === effect.id}
              index={index}
              key={effect.id}
              total={recipe.effects.length}
              onChange={(updatedEffect) =>
                onChange(
                  updateEffect(recipe, effect.id, () => updatedEffect),
                )
              }
              onMove={(offset) =>
                onChange(moveEffectByOffset(recipe, effect.id, offset))
              }
              onRemove={() => {
                onChange(removeEffect(recipe, effect.id));
              }}
              onReset={() => onChange(resetEffect(recipe, effect.id))}
              onToggle={(enabled) =>
                onChange(toggleEffect(recipe, effect.id, enabled))
              }
              onToggleExpanded={() =>
                setExpandedEffectId((current) =>
                  current === effect.id ? null : effect.id,
                )
              }
            />
          ))}
        </ol>
      )}
    </section>
  );
}

function EffectCard({
  disabled,
  effect,
  expanded,
  index,
  total,
  onChange,
  onMove,
  onRemove,
  onReset,
  onToggle,
  onToggleExpanded,
}: {
  disabled: boolean;
  effect: IconEffect;
  expanded: boolean;
  index: number;
  total: number;
  onChange: (effect: IconEffect) => void;
  onMove: (offset: -1 | 1) => void;
  onRemove: () => void;
  onReset: () => void;
  onToggle: (enabled: boolean) => void;
  onToggleExpanded: () => void;
}) {
  const panelId = useId();
  const label = effectKindLabel(effect.kind);

  return (
    <li data-effect-id={effect.id}>
      <article
        aria-label={`${index + 1}번째 효과 ${label}`}
        className={cn(
          "rounded-md border border-border bg-white",
          !effect.enabled && "opacity-70",
        )}
      >
        <div className="flex min-w-0 items-center gap-1.5 p-2">
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
            onClick={onToggleExpanded}
          >
            <span className="block truncate text-xs font-semibold">{label}</span>
            <span className="block truncate text-[11px] text-muted">
              {effect.enabled ? effectSummary(effect) : "사용 안 함"}
            </span>
          </button>
          <label className="flex shrink-0 items-center gap-1 text-[11px] text-muted">
            <input
              aria-label={`${label} 사용`}
              checked={effect.enabled}
              disabled={disabled}
              type="checkbox"
              onChange={(event) => onToggle(event.currentTarget.checked)}
            />
            사용
          </label>
          <button
            aria-label={`${label} 위로 이동`}
            className={iconButtonClass}
            disabled={disabled || index === 0}
            title="위로 이동"
            type="button"
            onClick={() => onMove(-1)}
          >
            ↑
          </button>
          <button
            aria-label={`${label} 아래로 이동`}
            className={iconButtonClass}
            disabled={disabled || index === total - 1}
            title="아래로 이동"
            type="button"
            onClick={() => onMove(1)}
          >
            ↓
          </button>
          <button
            aria-label={`${label} 제거`}
            className={iconButtonClass}
            disabled={disabled}
            title="효과 제거"
            type="button"
            onClick={onRemove}
          >
            ×
          </button>
        </div>

        {expanded ? (
          <div
            className="border-t border-border p-3"
            id={panelId}
            role="group"
          >
            <EffectParameters
              disabled={disabled}
              effect={effect}
              onChange={onChange}
            />
            <div className="mt-3 flex justify-end">
              <button
                className={smallButtonClass}
                disabled={disabled}
                title="이 효과의 매개변수만 권장 기본값으로 되돌립니다."
                type="button"
                onClick={onReset}
              >
                매개변수 기본값
              </button>
            </div>
          </div>
        ) : null}
      </article>
    </li>
  );
}

function EffectParameters({
  disabled,
  effect,
  onChange,
}: {
  disabled: boolean;
  effect: IconEffect;
  onChange: (effect: IconEffect) => void;
}) {
  switch (effect.kind) {
    case "pixelate":
      return (
        <RangeNumberField
          disabled={disabled}
          label="블록 크기"
          max={EFFECT_LIMITS.pixelateBlockSize.max}
          min={EFFECT_LIMITS.pixelateBlockSize.min}
          suffix="px"
          value={effect.blockSize}
          onChange={(blockSize) => onChange({ ...effect, blockSize })}
        />
      );
    case "color_adjust":
      return (
        <div className="grid gap-3 sm:grid-cols-2">
          <RangeNumberField
            disabled={disabled}
            label="밝기"
            max={EFFECT_LIMITS.colorAdjustment.max}
            min={EFFECT_LIMITS.colorAdjustment.min}
            value={effect.brightness}
            onChange={(brightness) => onChange({ ...effect, brightness })}
          />
          <RangeNumberField
            disabled={disabled}
            label="대비"
            max={EFFECT_LIMITS.colorAdjustment.max}
            min={EFFECT_LIMITS.colorAdjustment.min}
            value={effect.contrast}
            onChange={(contrast) => onChange({ ...effect, contrast })}
          />
          <RangeNumberField
            disabled={disabled}
            label="채도"
            max={EFFECT_LIMITS.colorAdjustment.max}
            min={EFFECT_LIMITS.colorAdjustment.min}
            value={effect.saturation}
            onChange={(saturation) => onChange({ ...effect, saturation })}
          />
          <RangeNumberField
            disabled={disabled}
            label="색조"
            max={EFFECT_LIMITS.hue.max}
            min={EFFECT_LIMITS.hue.min}
            suffix="°"
            value={effect.hue}
            onChange={(hue) => onChange({ ...effect, hue })}
          />
        </div>
      );
    case "tone":
      return (
        <div className="grid gap-3 sm:grid-cols-2">
          <label className="flex flex-col gap-1 text-xs font-medium text-muted">
            색감
            <select
              className={inputClass}
              disabled={disabled}
              value={effect.mode}
              onChange={(event) =>
                onChange({
                  ...effect,
                  mode:
                    event.currentTarget.value === "sepia"
                      ? "sepia"
                      : "grayscale",
                })
              }
            >
              <option value="grayscale">흑백</option>
              <option value="sepia">세피아</option>
            </select>
          </label>
          <RangeNumberField
            disabled={disabled}
            label="강도"
            max={EFFECT_LIMITS.toneAmount.max}
            min={EFFECT_LIMITS.toneAmount.min}
            suffix="%"
            value={effect.amount}
            onChange={(amount) => onChange({ ...effect, amount })}
          />
        </div>
      );
    case "blur":
      return (
        <RangeNumberField
          disabled={disabled}
          label="블러 반경"
          max={EFFECT_LIMITS.blurRadius.max}
          min={EFFECT_LIMITS.blurRadius.min}
          suffix="px"
          value={effect.radius}
          onChange={(radius) => onChange({ ...effect, radius })}
        />
      );
    case "sharpen":
      return (
        <RangeNumberField
          disabled={disabled}
          label="선명화 강도"
          max={EFFECT_LIMITS.sharpenAmount.max}
          min={EFFECT_LIMITS.sharpenAmount.min}
          suffix="%"
          value={effect.amount}
          onChange={(amount) => onChange({ ...effect, amount })}
        />
      );
    case "outline":
      return (
        <div className="grid gap-3 sm:grid-cols-2">
          <RangeNumberField
            disabled={disabled}
            label="윤곽선 두께"
            max={EFFECT_LIMITS.outlineRadius.max}
            min={EFFECT_LIMITS.outlineRadius.min}
            suffix="px"
            value={effect.radius}
            onChange={(radius) => onChange({ ...effect, radius })}
          />
          <ColorField
            color={effect.color}
            disabled={disabled}
            label="윤곽선 색"
            onChange={(color) => onChange({ ...effect, color })}
          />
        </div>
      );
    case "shadow":
      return (
        <div className="grid gap-3 sm:grid-cols-2">
          <RangeNumberField
            disabled={disabled}
            label="가로 거리"
            max={EFFECT_LIMITS.shadowOffset.max}
            min={EFFECT_LIMITS.shadowOffset.min}
            suffix="px"
            value={effect.offsetX}
            onChange={(offsetX) => onChange({ ...effect, offsetX })}
          />
          <RangeNumberField
            disabled={disabled}
            label="세로 거리"
            max={EFFECT_LIMITS.shadowOffset.max}
            min={EFFECT_LIMITS.shadowOffset.min}
            suffix="px"
            value={effect.offsetY}
            onChange={(offsetY) => onChange({ ...effect, offsetY })}
          />
          <RangeNumberField
            disabled={disabled}
            label="그림자 흐림"
            max={EFFECT_LIMITS.shadowBlurRadius.max}
            min={EFFECT_LIMITS.shadowBlurRadius.min}
            suffix="px"
            value={effect.blurRadius}
            onChange={(blurRadius) => onChange({ ...effect, blurRadius })}
          />
          <ColorField
            color={effect.color}
            disabled={disabled}
            label="그림자 색"
            onChange={(color) => onChange({ ...effect, color })}
          />
        </div>
      );
  }
}

function RangeNumberField({
  disabled,
  label,
  max,
  min,
  suffix,
  value,
  onChange,
}: {
  disabled: boolean;
  label: string;
  max: number;
  min: number;
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
      <div className="grid grid-cols-[minmax(80px,1fr)_72px_auto] items-center gap-2">
        <input
          aria-label={`${label} 슬라이더`}
          disabled={disabled}
          max={max}
          min={min}
          step={1}
          type="range"
          value={normalizedValue}
          onChange={(event) => onChange(Number(event.currentTarget.value))}
        />
        <input
          aria-label={`${label} 숫자 입력`}
          className="min-w-0 rounded-md border border-border bg-white px-2 py-1.5 text-sm text-foreground focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:text-muted"
          disabled={disabled}
          max={max}
          min={min}
          step={1}
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
  useEffect(() => {
    setDraftColor(color);
  }, [color]);
  const isValidDraft = /^#[0-9a-f]{6}(?:[0-9a-f]{2})?$/i.test(
    draftColor.trim(),
  );
  const validPickerColor = /^#[0-9a-f]{6}(?:[0-9a-f]{2})?$/i.test(color)
    ? color.slice(0, 7)
    : "#000000";
  const commitDraft = () => {
    if (isValidDraft) {
      const normalized = draftColor.trim().toLowerCase();
      setDraftColor(normalized);
      onChange(normalized);
      return;
    }
    setDraftColor(color);
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
          value={validPickerColor}
          onChange={(event) => {
            setDraftColor(event.currentTarget.value);
            onChange(event.currentTarget.value);
          }}
        />
        <input
          aria-label={`${label} 값`}
          aria-invalid={!isValidDraft}
          className={inputClass}
          disabled={disabled}
          maxLength={9}
          pattern="#[0-9A-Fa-f]{6}([0-9A-Fa-f]{2})?"
          spellCheck={false}
          value={draftColor}
          onBlur={commitDraft}
          onChange={(event) => setDraftColor(event.currentTarget.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              event.preventDefault();
              commitDraft();
            } else if (event.key === "Escape") {
              setDraftColor(color);
            }
          }}
        />
      </span>
      {!isValidDraft ? (
        <span className="text-[11px] font-normal text-danger">
          #RRGGBB 또는 #RRGGBBAA 형식으로 입력하세요.
        </span>
      ) : null}
    </label>
  );
}

const inputClass =
  "min-w-0 rounded-md border border-border bg-white px-2 py-2 text-sm text-foreground focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:text-muted";
const smallButtonClass =
  "rounded-md border border-border bg-white px-2.5 py-1.5 text-xs font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:text-muted";
const iconButtonClass =
  "flex size-7 shrink-0 items-center justify-center rounded border border-border bg-white text-sm font-semibold hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:text-muted";
