import {
  ExternalLink,
  Eye,
  EyeOff,
  KeyRound,
  LoaderCircle,
  Sparkles,
  Trash2,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { RefObject } from "react";

import {
  AI_PROVIDER_CHOICES,
  GEMINI_IMAGE_MODELS,
  NOVELAI_LIMITS,
  buildGeminiEditInput,
  buildNovelAiEditInput,
  consumeSessionCredential,

  createDefaultGeminiEditDraft,
  createDefaultNovelAiEditDraft,
  formatAiProviderExecutionError,
  geminiDraftErrors,
  novelAiDraftErrors,
  providerConfigured,
} from "@/features/editor/ai-provider-model";
import type {
  AiProviderChoice,
  GeminiEditDraft,
  NovelAiEditDraft,
} from "@/features/editor/ai-provider-model";
import {
  clearAiSessionCredential,
  deleteAiWebHandoffPayload,
  executeAiImageEdit,
  extendAiWebHandoffRetention,
  getAiProviderSessionStatus,
  getLatestAiWebHandoffForIcon,
  inspectAndCommitAiWebHandoffResult,
  openAiOfficialResource,
  prepareAiWebHandoff,
  revealAiWebHandoffUpload,
  setAiSessionCredential,
  startAiWebHandoffDrag,
} from "@/features/editor/api";
import { AiWebHandoffPanel } from "@/features/editor/components/AiWebHandoffPanel";
import type {
  AiOfficialResource,
  AiProvider,
  AiProviderSessionStatus,
  AiReviewState,
  SourceFileSummary,
} from "@/features/editor/types";
import { getCommandErrorMessage } from "@/lib/tauri";
import { cn } from "@/lib/utils";

type ProviderBusyAction =
  | `credential:${AiProvider}`
  | `clear:${AiProvider}`
  | `execute:${AiProvider}`
  | `resource:${AiOfficialResource}`;

interface AiProviderPanelProps {
  collectionId: string;
  iconId: string;
  source: SourceFileSummary;
  hasUnsavedChanges: boolean;
  disabled: boolean;
  onBusyStart: () => boolean;
  onBusyEnd: () => void;
  onGenerated: (reviewState: AiReviewState) => void;
  onAnnouncement: (message: string, tone: "status" | "error") => void;
  initialProviderChoice?: AiProviderChoice;
}

const EMPTY_SESSION_STATUS: AiProviderSessionStatus = {
  novelAiConfigured: false,
  geminiConfigured: false,
};

export function AiProviderPanel({
  collectionId,
  iconId,
  source,
  hasUnsavedChanges,
  disabled,
  onBusyStart,
  onBusyEnd,
  onGenerated,
  onAnnouncement,
  initialProviderChoice = "web",
}: AiProviderPanelProps) {
  const [providerChoice, setProviderChoice] =
    useState<AiProviderChoice>(initialProviderChoice);
  const [sessionStatus, setSessionStatus] =
    useState<AiProviderSessionStatus>(EMPTY_SESSION_STATUS);
  const [isStatusLoading, setIsStatusLoading] = useState(true);
  const [busyAction, setBusyAction] = useState<ProviderBusyAction | null>(null);
  const [showNovelAiSecret, setShowNovelAiSecret] = useState(false);
  const [showGeminiSecret, setShowGeminiSecret] = useState(false);
  const [novelAiDraft, setNovelAiDraft] = useState<NovelAiEditDraft>(
    createDefaultNovelAiEditDraft,
  );
  const [geminiDraft, setGeminiDraft] = useState<GeminiEditDraft>(
    createDefaultGeminiEditDraft,
  );

  const novelAiCredentialRef = useRef<HTMLInputElement>(null);
  const geminiCredentialRef = useRef<HTMLInputElement>(null);
  const restoreLatestWebHandoff = useCallback(
    () => getLatestAiWebHandoffForIcon(collectionId, iconId),
    [collectionId, iconId],
  );

  const executeInFlightRef = useRef(false);
  const mountedRef = useRef(true);
  const announcementRef = useRef(onAnnouncement);
  announcementRef.current = onAnnouncement;

  useEffect(() => {
    mountedRef.current = true;
    let active = true;
    void getAiProviderSessionStatus()
      .then((status) => {
        if (active) setSessionStatus(status);
      })
      .catch((error) => {
        if (active) {
          announcementRef.current(
            `세션 키 상태를 확인하지 못했습니다. ${getCommandErrorMessage(error)}`,
            "error",
          );
        }
      })
      .finally(() => {
        if (active) setIsStatusLoading(false);
      });
    return () => {
      active = false;
      mountedRef.current = false;
    };
  }, []);

  const novelAiErrors = useMemo(
    () => novelAiDraftErrors(novelAiDraft),
    [novelAiDraft],
  );
  const geminiErrors = useMemo(
    () => geminiDraftErrors(geminiDraft),
    [geminiDraft],
  );
  const controlsDisabled = disabled || busyAction !== null;
  const sourceBlockReason = source.isAnimated
    ? "이번 단계의 AI API·웹 바로 전달은 정적 JPG·PNG 소스만 지원합니다. GIF 프레임 스프라이트 왕복은 다음 업데이트에서 지원합니다."
    : null;

  const saveCredential = async (
    provider: AiProvider,
    inputRef: RefObject<HTMLInputElement | null>,
  ) => {
    const credential = consumeSessionCredential(inputRef.current);
    if (!credential) {
      onAnnouncement("세션 키를 입력해 주세요. 입력값은 저장 전에 즉시 비워집니다.", "error");
      inputRef.current?.focus();
      return;
    }
    if (!onBusyStart()) {
      onAnnouncement("다른 AI 작업이 끝난 뒤 다시 시도해 주세요.", "error");
      return;
    }
    const action = `credential:${provider}` as const;
    setBusyAction(action);
    try {
      const nextStatus = await setAiSessionCredential(provider, credential);
      if (!mountedRef.current) return;
      setSessionStatus(nextStatus);
      onAnnouncement(
        `${provider === "novelai" ? "NovelAI PAT" : "Gemini API 키"}를 이 앱 실행 세션에만 연결했습니다.`,
        "status",
      );
    } catch (error) {
      if (mountedRef.current) {
        onAnnouncement(getCommandErrorMessage(error), "error");
      }
    } finally {
      if (mountedRef.current) setBusyAction(null);
      onBusyEnd();
    }
  };

  const clearCredential = async (provider: AiProvider) => {
    if (!onBusyStart()) {
      onAnnouncement("다른 AI 작업이 끝난 뒤 다시 시도해 주세요.", "error");
      return;
    }
    const action = `clear:${provider}` as const;
    setBusyAction(action);
    try {
      const nextStatus = await clearAiSessionCredential(provider);
      if (!mountedRef.current) return;
      setSessionStatus(nextStatus);
      onAnnouncement(
        `${provider === "novelai" ? "NovelAI" : "Gemini"} 세션 키를 메모리에서 지웠습니다.`,
        "status",
      );
    } catch (error) {
      if (mountedRef.current) {
        onAnnouncement(getCommandErrorMessage(error), "error");
      }
    } finally {
      if (mountedRef.current) setBusyAction(null);
      onBusyEnd();
    }
  };

  const executeProviderEdit = async (provider: AiProvider) => {
    if (executeInFlightRef.current || controlsDisabled) return;
    const errors =
      provider === "novelai" ? novelAiErrors : geminiErrors;
    if (!providerConfigured(provider, sessionStatus)) {
      onAnnouncement(
        `${provider === "novelai" ? "NovelAI PAT" : "Gemini API 키"}를 먼저 이 세션에 연결해 주세요.`,
        "error",
      );
      return;
    }
    if (sourceBlockReason) {
      onAnnouncement(sourceBlockReason, "error");
      return;
    }
    if (errors.length > 0) {
      onAnnouncement(errors[0] ?? "필수 확인 항목을 확인해 주세요.", "error");
      return;
    }
    if (!onBusyStart()) {
      onAnnouncement("다른 AI 작업이 끝난 뒤 다시 시도해 주세요.", "error");
      return;
    }

    executeInFlightRef.current = true;
    const action = `execute:${provider}` as const;
    setBusyAction(action);
    try {
      const payload =
        provider === "novelai"
          ? buildNovelAiEditInput(iconId, novelAiDraft)
          : buildGeminiEditInput(iconId, geminiDraft);
      const nextState = await executeAiImageEdit(collectionId, payload);
      if (!mountedRef.current) return;
      onGenerated(nextState);
      onAnnouncement(
        `${provider === "novelai" ? "NovelAI" : "Gemini"} 결과 1장을 비활성 후보로 가져왔습니다. 원본은 바뀌지 않았습니다.`,
        "status",
      );
    } catch (error) {
      if (mountedRef.current) {
        onAnnouncement(
          formatAiProviderExecutionError(provider, getCommandErrorMessage(error)),
          "error",
        );
      }
    } finally {
      executeInFlightRef.current = false;
      if (mountedRef.current) setBusyAction(null);
      onBusyEnd();
    }
  };

  const openOfficialResource = async (resource: AiOfficialResource) => {
    if (busyAction !== null) return;
    const action = `resource:${resource}` as const;
    setBusyAction(action);
    try {
      await openAiOfficialResource(resource);
      if (mountedRef.current) {
        onAnnouncement("백엔드가 확인한 공식 주소를 기본 브라우저에서 열었습니다.", "status");
      }
    } catch (error) {
      if (mountedRef.current) {
        onAnnouncement(getCommandErrorMessage(error), "error");
      }
    } finally {
      if (mountedRef.current) setBusyAction(null);
    }
  };



  return (
    <section
      aria-labelledby="ai-provider-panel-title"
      className="flex flex-col gap-4 rounded-md border border-border bg-white p-4"
      data-testid="ai-provider-panel"
    >
      <div className="flex gap-2">
        <Sparkles aria-hidden="true" className="mt-0.5 size-4 shrink-0 text-focus" />
        <div>
          <h3 className="text-sm font-semibold" id="ai-provider-panel-title">
            AI 수정 또는 웹 전달
          </h3>
          <p className="mt-1 text-xs leading-5 text-muted">
            키는 디스크·DB·기록에 저장하지 않고 현재 앱 실행 중 Rust 메모리에만 둡니다.
            생성은 사람의 버튼 클릭 1회당 이미지 1장만 요청하며 자동 재시도하지 않습니다.
          </p>
        </div>
      </div>

      <div className="rounded-md border border-border bg-preview p-3 text-xs leading-5">
        <p className="font-semibold">AI에 전달할 저장된 기준 소스</p>
        <p className="mt-1 text-muted">
          {source.originalFilename} · {source.width}×{source.height}
          {source.isAnimated ? ` · GIF ${source.frameCount ?? "?"}프레임` : ""}
        </p>
        <p className="mt-1 text-muted">
          저장된 현재 유효 소스 파일의 바이트를 전송합니다. 저장하지 않은 crop·변환·텍스트·효과·
          모션과 최종 렌더 미리보기는 전송하지 않습니다.
        </p>
        {hasUnsavedChanges ? (
          <p className="mt-1 font-semibold text-warning">
            저장하지 않은 편집이 있어 화면과 전송 소스가 다를 수 있습니다.
          </p>
        ) : null}
        {sourceBlockReason ? (
          <p className="mt-1 font-semibold text-danger">{sourceBlockReason}</p>
        ) : null}
      </div>
      <fieldset className="grid gap-2 sm:grid-cols-3">
        <legend className="mb-2 text-xs font-semibold">사용 방식</legend>
        {AI_PROVIDER_CHOICES.map((choice) => (
          <label
            className={cn(
              "flex cursor-pointer flex-col gap-1 rounded-md border p-3 text-xs",
              providerChoice === choice.value
                ? "border-focus bg-selected/40"
                : "border-border bg-white hover:bg-menu-hover",
              controlsDisabled && "cursor-not-allowed opacity-60",
            )}
            key={choice.value}
          >
            <span className="flex items-center gap-2 font-semibold">
              <input
                checked={providerChoice === choice.value}
                disabled={controlsDisabled}
                name="ai-provider-choice"
                type="radio"
                value={choice.value}
                onChange={() => setProviderChoice(choice.value)}
              />
              {choice.label}
            </span>
            <span className="leading-4 text-muted">{choice.description}</span>
          </label>
        ))}
      </fieldset>

      {providerChoice === "novelai" ? (
        <NovelAiForm
          busyAction={busyAction}
          configured={sessionStatus.novelAiConfigured}
          credentialRef={novelAiCredentialRef}
          disabled={controlsDisabled || sourceBlockReason !== null}
          draft={novelAiDraft}
          errors={novelAiErrors}
          isStatusLoading={isStatusLoading}
          showSecret={showNovelAiSecret}
          onClearCredential={() => {
            void clearCredential("novelai");
          }}
          onDraftChange={setNovelAiDraft}
          onExecute={() => {
            void executeProviderEdit("novelai");
          }}
          onOpenResource={(resource) => {
            void openOfficialResource(resource);
          }}
          onSaveCredential={() => {
            void saveCredential("novelai", novelAiCredentialRef);
          }}
          onShowSecretChange={setShowNovelAiSecret}
        />
      ) : null}

      {providerChoice === "gemini" ? (
        <GeminiForm
          busyAction={busyAction}
          configured={sessionStatus.geminiConfigured}
          credentialRef={geminiCredentialRef}
          disabled={controlsDisabled || sourceBlockReason !== null}
          draft={geminiDraft}
          errors={geminiErrors}
          isStatusLoading={isStatusLoading}
          showSecret={showGeminiSecret}
          onClearCredential={() => {
            void clearCredential("gemini");
          }}
          onDraftChange={setGeminiDraft}
          onExecute={() => {
            void executeProviderEdit("gemini");
          }}
          onOpenResource={(resource) => {
            void openOfficialResource(resource);
          }}
          onSaveCredential={() => {
            void saveCredential("gemini", geminiCredentialRef);
          }}
          onShowSecretChange={setShowGeminiSecret}
        />
      ) : null}

      {providerChoice === "web" ? (
        <AiWebHandoffPanel
          disabled={controlsDisabled || sourceBlockReason !== null}
          hasUnsavedChanges={hasUnsavedChanges}
          onAnnouncement={onAnnouncement}
          onBusyEnd={onBusyEnd}
          onBusyStart={onBusyStart}
          onCommitResult={inspectAndCommitAiWebHandoffResult}
          onCommitted={(result) => {
            if (result.reviewState) onGenerated(result.reviewState);
          }}
          onExtendRetention={extendAiWebHandoffRetention}
          onDeleteSession={deleteAiWebHandoffPayload}
          onOpenSite={(serviceSurface) =>
            openAiOfficialResource(
              serviceSurface === "gemini_web"
                ? "gemini_ai_studio"
                : "novelai_app",
            )
          }
          onRestoreLatest={restoreLatestWebHandoff}
          onPrepare={(serviceSurface, userPrompt) =>
            prepareAiWebHandoff(
              collectionId,
              iconId,
              serviceSurface,
              userPrompt,
            )
          }
          onRevealUpload={revealAiWebHandoffUpload}
          onStartNativeDrag={startAiWebHandoffDrag}
        />
      ) : null}
    </section>
  );
}

function SessionCredentialControl({
  configured,
  credentialRef,
  disabled,
  isLoading,
  label,
  provider,
  showSecret,
  onClear,
  onSave,
  onShowSecretChange,
}: {
  configured: boolean;
  credentialRef: RefObject<HTMLInputElement | null>;
  disabled: boolean;
  isLoading: boolean;
  label: string;
  provider: AiProvider;
  showSecret: boolean;
  onClear: () => void;
  onSave: () => void;
  onShowSecretChange: (show: boolean) => void;
}) {
  const inputId = `ai-${provider}-credential`;
  return (
    <div className="rounded-md border border-border bg-preview p-3">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <label className="text-xs font-semibold" htmlFor={inputId}>
          {label}
        </label>
        <span
          className={cn(
            "rounded-full px-2 py-1 text-[11px] font-semibold",
            configured ? "bg-success/10 text-success" : "bg-white text-muted",
          )}
          data-testid={`ai-${provider}-credential-status`}
        >
          {isLoading ? "상태 확인 중" : configured ? "이 세션에 연결됨" : "연결되지 않음"}
        </span>
      </div>
      <div className="mt-2 flex flex-col gap-2 sm:flex-row">
        <div className="relative min-w-0 flex-1">
          <input
            aria-describedby={`${inputId}-help`}
            autoComplete="off"
            className="w-full rounded-md border border-border bg-white px-3 py-2 pr-10 text-sm focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
            data-testid={inputId}
            disabled={disabled}
            id={inputId}
            ref={credentialRef}
            spellCheck={false}
            type={showSecret ? "text" : "password"}
          />
          <button
            aria-label={showSecret ? `${label} 숨기기` : `${label} 보기`}
            className="absolute inset-y-0 right-0 inline-flex w-10 items-center justify-center text-muted hover:text-foreground focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
            disabled={disabled}
            type="button"
            onClick={() => onShowSecretChange(!showSecret)}
          >
            {showSecret ? (
              <EyeOff aria-hidden="true" className="size-4" />
            ) : (
              <Eye aria-hidden="true" className="size-4" />
            )}
          </button>
        </div>
        <button
          className="inline-flex min-h-10 items-center justify-center gap-2 rounded-md border border-border bg-white px-3 py-2 text-xs font-semibold hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:opacity-50"
          disabled={disabled}
          type="button"
          onClick={onSave}
        >
          <KeyRound aria-hidden="true" className="size-4" />
          세션에 연결
        </button>
        <button
          aria-label={`${label} 세션에서 지우기`}
          className="inline-flex min-h-10 items-center justify-center gap-2 rounded-md border border-border bg-white px-3 py-2 text-xs font-semibold hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:opacity-50"
          disabled={disabled || !configured}
          type="button"
          onClick={onClear}
        >
          <Trash2 aria-hidden="true" className="size-4" />
          지우기
        </button>
      </div>
      <p className="mt-2 text-[11px] leading-4 text-muted" id={`${inputId}-help`}>
        입력은 명령 호출 전에 즉시 비워지며 화면·DB·로그·후보 기록에 다시 표시되지 않습니다.
        앱을 종료하면 세션 키도 사라집니다.
      </p>
    </div>
  );
}

function NovelAiForm({
  busyAction,
  configured,
  credentialRef,
  disabled,
  draft,
  errors,
  isStatusLoading,
  showSecret,
  onClearCredential,
  onDraftChange,
  onExecute,
  onOpenResource,
  onSaveCredential,
  onShowSecretChange,
}: {
  busyAction: ProviderBusyAction | null;
  configured: boolean;
  credentialRef: RefObject<HTMLInputElement | null>;
  disabled: boolean;
  draft: NovelAiEditDraft;
  errors: string[];
  isStatusLoading: boolean;
  showSecret: boolean;
  onClearCredential: () => void;
  onDraftChange: (draft: NovelAiEditDraft) => void;
  onExecute: () => void;
  onOpenResource: (resource: AiOfficialResource) => void;
  onSaveCredential: () => void;
  onShowSecretChange: (show: boolean) => void;
}) {
  const ready = configured && errors.length === 0;
  return (
    <div className="flex flex-col gap-3" data-testid="ai-novelai-form">
      <div className="flex flex-wrap gap-2">
        <OfficialResourceButton disabled={disabled} label="PAT 발급" resource="novelai_pat" onOpen={onOpenResource} />
        <OfficialResourceButton disabled={disabled} label="API 문서" resource="novelai_docs" onOpen={onOpenResource} />
        <OfficialResourceButton disabled={disabled} label="이용약관" resource="novelai_terms" onOpen={onOpenResource} />
      </div>
      <SessionCredentialControl
        configured={configured}
        credentialRef={credentialRef}
        disabled={disabled}
        isLoading={isStatusLoading}
        label="NovelAI PAT"
        provider="novelai"
        showSecret={showSecret}
        onClear={onClearCredential}
        onSave={onSaveCredential}
        onShowSecretChange={onShowSecretChange}
      />
      <LabeledTextarea
        disabled={disabled}
        id="ai-novelai-prompt"
        label="수정 프롬프트"
        value={draft.prompt}
        onChange={(prompt) => onDraftChange({ ...draft, prompt })}
      />
      <LabeledTextarea
        disabled={disabled}
        id="ai-novelai-negative-prompt"
        label="제외 프롬프트 (선택)"
        value={draft.negativePrompt}
        onChange={(negativePrompt) => onDraftChange({ ...draft, negativePrompt })}
      />
      <details className="rounded-md border border-warning/30 bg-warning/5 p-3">
        <summary className="cursor-pointer text-xs font-semibold">
          고급 API 계약 값 · 직접 확인 필요
        </summary>
        <p className="mt-2 text-xs leading-5 text-muted" id="ai-novelai-contract-warning">
          NovelAI 공식 OpenAPI는 현재 사용 가능한 모델 ID와 action 전체 enum을 공개하지
          않습니다. 아래 값은 기본값을 제공하지 않으며, 공식 문서와 계정에서 직접 확인한
          정확한 값을 입력해야 합니다. PMTCONCON Studio가 호환성을 보장하지 않습니다.
        </p>
        <div className="mt-3 grid gap-3 sm:grid-cols-2">
          <LabeledTextInput
            describedBy="ai-novelai-contract-warning"
            disabled={disabled}
            id="ai-novelai-model"
            label="모델 ID (정확한 값)"
            placeholder="공식 문서/계정에서 확인"
            value={draft.model}
            onChange={(model) => onDraftChange({ ...draft, model })}
          />
          <LabeledTextInput
            describedBy="ai-novelai-contract-warning"
            disabled={disabled}
            id="ai-novelai-action"
            label="action (정확한 값)"
            placeholder="공식 계약에서 확인"
            value={draft.action}
            onChange={(action) => onDraftChange({ ...draft, action })}
          />
          <NumberField disabled={disabled} label="너비" limit={NOVELAI_LIMITS.width} value={draft.width} onChange={(width) => onDraftChange({ ...draft, width })} />
          <NumberField disabled={disabled} label="높이" limit={NOVELAI_LIMITS.height} value={draft.height} onChange={(height) => onDraftChange({ ...draft, height })} />
          <NumberField disabled={disabled} label="스텝" limit={NOVELAI_LIMITS.steps} value={draft.steps} onChange={(steps) => onDraftChange({ ...draft, steps })} />
          <NumberField disabled={disabled} label="scale" limit={NOVELAI_LIMITS.scale} value={draft.scale} onChange={(scale) => onDraftChange({ ...draft, scale })} />
          <NumberField disabled={disabled} label="strength" limit={NOVELAI_LIMITS.strength} value={draft.strength} onChange={(strength) => onDraftChange({ ...draft, strength })} />
          <NumberField disabled={disabled} label="noise" limit={NOVELAI_LIMITS.noise} value={draft.noise} onChange={(noise) => onDraftChange({ ...draft, noise })} />
        </div>
      </details>
      <fieldset className="grid gap-2 rounded-md border border-border p-3">
        <legend className="px-1 text-xs font-semibold">요청 전 확인</legend>
        <ConsentCheckbox checked={draft.humanActionConfirmed} label="사람이 지금 직접 시작하는 1회 요청이며 배치·백그라운드 실행이 아닙니다." onChange={(humanActionConfirmed) => onDraftChange({ ...draft, humanActionConfirmed })} />
        <ConsentCheckbox checked={draft.rightsConfirmed} label="현재 이미지와 프롬프트를 사용할 권리가 있습니다." onChange={(rightsConfirmed) => onDraftChange({ ...draft, rightsConfirmed })} />
        <ConsentCheckbox checked={draft.costConfirmed} label="Image Anlas 또는 구독 사용량이 들 수 있음을 확인했습니다." onChange={(costConfirmed) => onDraftChange({ ...draft, costConfirmed })} />
        <ConsentCheckbox checked={draft.requestContentConfirmed} label="현재 이미지와 프롬프트가 NovelAI로 전송됨을 확인했습니다." onChange={(requestContentConfirmed) => onDraftChange({ ...draft, requestContentConfirmed })} />
        <ConsentCheckbox checked={draft.contractOverrideConfirmed} label="모델 ID/action의 미공개·변경 가능 계약을 직접 확인했고 실패 가능성을 감수합니다." onChange={(contractOverrideConfirmed) => onDraftChange({ ...draft, contractOverrideConfirmed })} />
      </fieldset>
      <ProviderReadiness errors={errors} id="ai-novelai-readiness" configured={configured} />
      <button
        aria-busy={busyAction === "execute:novelai"}
        aria-describedby="ai-novelai-readiness"
        className="inline-flex min-h-10 items-center justify-center gap-2 rounded-md bg-accent px-3 py-2 text-xs font-semibold text-accent-foreground hover:bg-accent-strong focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:opacity-50"
        data-testid="ai-novelai-execute"
        disabled={disabled || !ready}
        type="button"
        onClick={onExecute}
      >
        {busyAction === "execute:novelai" ? (
          <LoaderCircle aria-hidden="true" className="size-4 animate-spin motion-reduce:animate-none" />
        ) : (
          <Sparkles aria-hidden="true" className="size-4" />
        )}
        이 이미지 1장 수정
      </button>
    </div>
  );
}

function GeminiForm({
  busyAction,
  configured,
  credentialRef,
  disabled,
  draft,
  errors,
  isStatusLoading,
  showSecret,
  onClearCredential,
  onDraftChange,
  onExecute,
  onOpenResource,
  onSaveCredential,
  onShowSecretChange,
}: {
  busyAction: ProviderBusyAction | null;
  configured: boolean;
  credentialRef: RefObject<HTMLInputElement | null>;
  disabled: boolean;
  draft: GeminiEditDraft;
  errors: string[];
  isStatusLoading: boolean;
  showSecret: boolean;
  onClearCredential: () => void;
  onDraftChange: (draft: GeminiEditDraft) => void;
  onExecute: () => void;
  onOpenResource: (resource: AiOfficialResource) => void;
  onSaveCredential: () => void;
  onShowSecretChange: (show: boolean) => void;
}) {
  const ready = configured && errors.length === 0;
  return (
    <div className="flex flex-col gap-3" data-testid="ai-gemini-form">
      <div className="rounded-md border border-warning/30 bg-warning/5 p-3 text-xs leading-5">
        <p className="font-semibold">실험실 · 비공개 파일럿</p>
        <p className="mt-1 text-muted">
          일반 소비자용 무료 기능을 약속하지 않습니다. 현재 이미지 모델은 유료이며,
          연령·전문/사업 목적·지원 지역·Paid Services 조건을 사용자가 직접 확인해야 합니다.
        </p>
      </div>
      <div className="flex flex-wrap gap-2">
        <OfficialResourceButton disabled={disabled} label="AI Studio" resource="gemini_ai_studio" onOpen={onOpenResource} />
        <OfficialResourceButton disabled={disabled} label="이미지 API 문서" resource="gemini_image_docs" onOpen={onOpenResource} />
        <OfficialResourceButton disabled={disabled} label="가격" resource="gemini_pricing" onOpen={onOpenResource} />
        <OfficialResourceButton disabled={disabled} label="추가 약관" resource="gemini_terms" onOpen={onOpenResource} />
      </div>
      <SessionCredentialControl
        configured={configured}
        credentialRef={credentialRef}
        disabled={disabled}
        isLoading={isStatusLoading}
        label="Gemini API 키"
        provider="gemini"
        showSecret={showSecret}
        onClear={onClearCredential}
        onSave={onSaveCredential}
        onShowSecretChange={onShowSecretChange}
      />
      <LabeledTextarea
        disabled={disabled}
        id="ai-gemini-prompt"
        label="수정 프롬프트"
        value={draft.prompt}
        onChange={(prompt) => onDraftChange({ ...draft, prompt })}
      />
      <div className="flex flex-col gap-1">
        <label className="text-xs font-semibold" htmlFor="ai-gemini-model">이미지 모델</label>
        <select
          className="rounded-md border border-border bg-white px-3 py-2 text-sm focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
          disabled={disabled}
          id="ai-gemini-model"
          value={draft.model}
          onChange={(event) =>
            onDraftChange({
              ...draft,
              model: event.currentTarget.value as GeminiEditDraft["model"],
            })
          }
        >
          {GEMINI_IMAGE_MODELS.map((model) => <option key={model} value={model}>{model}</option>)}
        </select>
      </div>
      <fieldset className="grid gap-2 rounded-md border border-border p-3">
        <legend className="px-1 text-xs font-semibold">자격·전송·비용 확인</legend>
        <ConsentCheckbox checked={draft.humanActionConfirmed} label="사람이 지금 직접 시작하는 1회 요청이며 배치·백그라운드 실행이 아닙니다." onChange={(humanActionConfirmed) => onDraftChange({ ...draft, humanActionConfirmed })} />
        <ConsentCheckbox checked={draft.rightsConfirmed} label="현재 이미지와 프롬프트를 사용할 권리가 있습니다." onChange={(rightsConfirmed) => onDraftChange({ ...draft, rightsConfirmed })} />
        <ConsentCheckbox checked={draft.requestContentConfirmed} label="현재 이미지와 프롬프트가 Google로 전송됨을 확인했습니다." onChange={(requestContentConfirmed) => onDraftChange({ ...draft, requestContentConfirmed })} />
        <ConsentCheckbox checked={draft.costConfirmed} label="선택한 모델의 유료 호출 비용을 확인했습니다." onChange={(costConfirmed) => onDraftChange({ ...draft, costConfirmed })} />
        <ConsentCheckbox checked={draft.adultConfirmed} label="사용자는 만 18세 이상입니다." onChange={(adultConfirmed) => onDraftChange({ ...draft, adultConfirmed })} />
        <ConsentCheckbox checked={draft.under18AudienceExcludedConfirmed} label="이 API 클라이언트는 미성년자를 대상으로 하거나 미성년자가 접근할 가능성이 있는 용도가 아닙니다." onChange={(under18AudienceExcludedConfirmed) => onDraftChange({ ...draft, under18AudienceExcludedConfirmed })} />
        <ConsentCheckbox checked={draft.professionalBusinessConfirmed} label="전문적 또는 사업 목적의 개발 사용입니다." onChange={(professionalBusinessConfirmed) => onDraftChange({ ...draft, professionalBusinessConfirmed })} />
        <ConsentCheckbox checked={draft.supportedRegionConfirmed} label="현재 사용·배포 지역이 지원 대상인지 확인했습니다." onChange={(supportedRegionConfirmed) => onDraftChange({ ...draft, supportedRegionConfirmed })} />
        <ConsentCheckbox checked={draft.paidServiceConfirmed} label="Paid Services 및 해당 데이터 처리 조건을 확인했습니다." onChange={(paidServiceConfirmed) => onDraftChange({ ...draft, paidServiceConfirmed })} />
      </fieldset>
      <ProviderReadiness errors={errors} id="ai-gemini-readiness" configured={configured} />
      <button
        aria-busy={busyAction === "execute:gemini"}
        aria-describedby="ai-gemini-readiness"
        className="inline-flex min-h-10 items-center justify-center gap-2 rounded-md bg-accent px-3 py-2 text-xs font-semibold text-accent-foreground hover:bg-accent-strong focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:opacity-50"
        data-testid="ai-gemini-execute"
        disabled={disabled || !ready}
        type="button"
        onClick={onExecute}
      >
        {busyAction === "execute:gemini" ? (
          <LoaderCircle aria-hidden="true" className="size-4 animate-spin motion-reduce:animate-none" />
        ) : (
          <Sparkles aria-hidden="true" className="size-4" />
        )}
        이 이미지 1장 수정
      </button>
    </div>
  );
}

function ProviderReadiness({
  configured,
  errors,
  id,
}: {
  configured: boolean;
  errors: string[];
  id: string;
}) {
  return (
    <div className="rounded-md border border-border bg-preview p-3 text-xs leading-5" id={id}>
      {configured && errors.length === 0 ? (
        <p className="font-semibold text-success">1장 요청 준비가 끝났습니다.</p>
      ) : (
        <>
          <p className="font-semibold">아직 요청할 수 없습니다.</p>
          <p className="text-muted">
            {!configured ? "세션 키 연결이 필요합니다. " : ""}
            {errors[0] ?? ""}
          </p>
        </>
      )}
    </div>
  );
}

function ConsentCheckbox({
  checked,
  label,
  onChange,
}: {
  checked: boolean;
  label: string;
  onChange: (checked: boolean) => void;
}) {
  return (
    <label className="flex items-start gap-2 text-xs leading-5">
      <input
        checked={checked}
        className="mt-1"
        type="checkbox"
        onChange={(event) => onChange(event.currentTarget.checked)}
      />
      <span>{label}</span>
    </label>
  );
}

function OfficialResourceButton({
  disabled,
  label,
  resource,
  onOpen,
}: {
  disabled: boolean;
  label: string;
  resource: AiOfficialResource;
  onOpen: (resource: AiOfficialResource) => void;
}) {
  return (
    <button
      className="inline-flex min-h-9 items-center justify-center gap-2 rounded-md border border-border bg-white px-3 py-2 text-xs font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:opacity-50"
      data-resource={resource}
      disabled={disabled}
      type="button"
      onClick={() => onOpen(resource)}
    >
      <ExternalLink aria-hidden="true" className="size-3.5" />
      {label}
    </button>
  );
}

function LabeledTextarea({
  disabled = false,
  id,
  label,
  textareaRef,
  value,
  onChange,
}: {
  disabled?: boolean;
  id: string;
  label: string;
  textareaRef?: RefObject<HTMLTextAreaElement | null>;
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <div className="flex flex-col gap-1">
      <label className="text-xs font-semibold" htmlFor={id}>{label}</label>
      <textarea
        disabled={disabled}
        className="min-h-24 resize-y rounded-md border border-border bg-white px-3 py-2 text-sm leading-5 focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
        id={id}
        ref={textareaRef}
        value={value}
        onChange={(event) => onChange(event.currentTarget.value)}
      />
    </div>
  );
}

function LabeledTextInput({
  disabled = false,
  describedBy,
  id,
  label,
  placeholder,
  value,
  onChange,
}: {
  disabled?: boolean;
  describedBy?: string;
  id: string;
  label: string;
  placeholder?: string;
  value: string;
  onChange: (value: string) => void;
}) {
  return (
    <div className="flex flex-col gap-1">
      <label className="text-xs font-semibold" htmlFor={id}>{label}</label>
      <input
        aria-describedby={describedBy}
        disabled={disabled}
        className="rounded-md border border-border bg-white px-3 py-2 text-sm focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
        id={id}
        placeholder={placeholder}
        type="text"
        value={value}
        onChange={(event) => onChange(event.currentTarget.value)}
      />
    </div>
  );
}

function NumberField({
  disabled = false,
  label,
  limit,
  value,
  onChange,
}: {
  disabled?: boolean;
  label: string;
  limit: { min: number; max: number; step: number };
  value: number;
  onChange: (value: number) => void;
}) {
  const id = `ai-novelai-${label.replace(/[^a-z0-9]/gi, "-").toLowerCase()}`;
  return (
    <div className="flex flex-col gap-1">
      <label className="text-xs font-semibold" htmlFor={id}>{label}</label>
      <input
        disabled={disabled}
        className="rounded-md border border-border bg-white px-3 py-2 text-sm focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
        id={id}
        max={limit.max}
        min={limit.min}
        step={limit.step}
        type="number"
        value={Number.isFinite(value) ? value : ""}
        onChange={(event) => onChange(event.currentTarget.valueAsNumber)}
      />
    </div>
  );
}
