import {
  AlertTriangle,
  Clock3,
  Copy,
  ExternalLink,
  FolderOpen,
  GripVertical,
  ImagePlus,
  LoaderCircle,
  ShieldCheck,
  Trash2,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { DragEvent, KeyboardEvent, MouseEvent, RefObject } from "react";

import { NovelAiWebGuide } from "@/features/ai-web/components/NovelAiWebGuide";
import { needsNovelAiEnglishInputHint } from "@/features/ai-web/novelai-web-model";

import {
  AI_WEB_HANDOFF_RESULT_ACCEPT,
  buildCombinedAiWebHandoffCorrectionPrompt,
  classifyPastedWebError,
  describeAiWebHandoffIssue,
  selectAiWebHandoffResultFile,
} from "@/features/editor/ai-web-handoff-model";
import { copyAiHandoffPrompt } from "@/features/editor/ai-provider-model";
import type {
  AiWebHandoffDeleteResult,
  AiWebHandoffDragResult,
  AiWebHandoffResultInspection,
  AiWebHandoffServiceSurface,
  AiWebHandoffSession,
} from "@/features/editor/types";
import { CommandError, getCommandErrorMessage } from "@/lib/tauri";
import { cn } from "@/lib/utils";

type WebServiceSurface = AiWebHandoffServiceSurface;
type WorkingAction = "prepare" | "reveal" | "drag" | "commit" | "extend" | "delete";
type CopyState = "idle" | "copied" | "fallback" | "failed";

export interface AiWebHandoffPanelProps {
  disabled: boolean;
  hasUnsavedChanges: boolean;
  onBusyStart: () => boolean;
  onBusyEnd: () => void;
  onAnnouncement: (message: string, tone: "status" | "error") => void;
  onPrepare: (
    serviceSurface: WebServiceSurface,
    userPrompt: string,
  ) => Promise<AiWebHandoffSession>;
  onRestoreLatest: () => Promise<AiWebHandoffSession | null>;
  onOpenSite: (serviceSurface: WebServiceSurface) => Promise<void>;
  onRevealUpload: (requestId: string) => Promise<void>;
  onStartNativeDrag: (requestId: string) => Promise<AiWebHandoffDragResult>;
  onExtendRetention: (requestId: string) => Promise<AiWebHandoffSession>;
  onDeleteSession: (requestId: string) => Promise<AiWebHandoffDeleteResult>;
  onCommitResult: (
    requestId: string,
    file: File,
  ) => Promise<AiWebHandoffResultInspection>;
  onCommitted: (result: AiWebHandoffResultInspection) => void;
}

const WEB_SERVICE_OPTIONS: ReadonlyArray<{
  value: WebServiceSurface;
  label: string;
}> = [
  { value: "gemini_web", label: "Gemini 웹" },
  { value: "novelai_web", label: "NovelAI 웹" },
];

const errorMessage = getCommandErrorMessage;
const CLOSED_SESSION_ERROR_CODES = new Set([
  "ai_handoff_stale",
  "ai_handoff_expired",
  "ai_handoff_payload_deleted",
  "ai_handoff_closed",
]);

export function AiWebHandoffPanel({
  disabled,
  hasUnsavedChanges,
  onBusyStart,
  onBusyEnd,
  onAnnouncement,
  onPrepare,
  onRestoreLatest,
  onOpenSite,
  onRevealUpload,
  onStartNativeDrag,
  onExtendRetention,
  onDeleteSession,
  onCommitResult,
  onCommitted,
}: AiWebHandoffPanelProps) {
  const [serviceSurface, setServiceSurface] =
    useState<WebServiceSurface>("gemini_web");
  const [userPrompt, setUserPrompt] = useState("");
  const [session, setSession] = useState<AiWebHandoffSession | null>(null);
  const [preparedUserPrompt, setPreparedUserPrompt] = useState<string | null>(null);
  const [restoredSessionUntouched, setRestoredSessionUntouched] =
    useState(false);
  const [completedResult, setCompletedResult] =
    useState<AiWebHandoffResultInspection | null>(null);
  const [workingAction, setWorkingAction] = useState<WorkingAction | null>(
    null,
  );
  const [isRestoring, setIsRestoring] = useState(true);
  const [promptCopyState, setPromptCopyState] = useState<CopyState>("idle");
  const [promptCopyRevision, setPromptCopyRevision] = useState(0);
  const [resultFile, setResultFile] = useState<File | null>(null);
  const [resultError, setResultError] = useState<string | null>(null);
  const [commitResult, setCommitResult] =
    useState<AiWebHandoffResultInspection | null>(null);
  const [isDraggingOver, setIsDraggingOver] = useState(false);
  const [webErrorText, setWebErrorText] = useState("");
  const announcementRef = useRef(onAnnouncement);
  announcementRef.current = onAnnouncement;
  const uploadPromptRef = useRef<HTMLTextAreaElement>(null);
  const correctionPromptRef = useRef<HTMLTextAreaElement>(null);
  const resultInputRef = useRef<HTMLInputElement>(null);
  const promptCopyGenerationRef = useRef(0);

  const correctionPrompt = useMemo(
    () =>
      buildCombinedAiWebHandoffCorrectionPrompt(
        commitResult?.issues ?? [],
      ),
    [commitResult],
  );
  const pastedWebErrorGuidance = useMemo(
    () => classifyPastedWebError(webErrorText),
    [webErrorText],
  );
  const controlsDisabled = disabled || workingAction !== null || isRestoring;
  const isNovelAi = serviceSurface === "novelai_web";
  const expectedCanvasLabel = session
    ? `${session.expectedWidth}×${session.expectedHeight}px`
    : "현재 아이콘의 목표 크기";
  const prepareBlocked = controlsDisabled || !userPrompt.trim();
  const sessionMatchesDraft =
    session === null ||
    (session.serviceSurface === serviceSurface &&
      (preparedUserPrompt === userPrompt.trim() ||
        (preparedUserPrompt === null && restoredSessionUntouched)));

  const resetPromptCopySequence = useCallback(() => {
    promptCopyGenerationRef.current += 1;
    setPromptCopyState("idle");
    setPromptCopyRevision((revision) => revision + 1);
  }, []);

  useEffect(() => {
    let active = true;
    setIsRestoring(true);
    void onRestoreLatest()
      .then((restored) => {
        if (!active) return;
        if (!restored) {
          setSession(null);
          setPreparedUserPrompt(null);
          setRestoredSessionUntouched(false);
          setCommitResult(null);
          setResultFile(null);
          setResultError(null);
          resetPromptCopySequence();
          return;
        }
        setSession(restored);
        setServiceSurface(restored.serviceSurface);
        setPreparedUserPrompt(null);
        setRestoredSessionUntouched(true);
        resetPromptCopySequence();
        announcementRef.current(
          "이 아이콘에서 진행 중이던 웹 전달을 다시 불러왔습니다.",
          "status",
        );
      })
      .catch((error) => {
        if (active) {
          if (
            error instanceof CommandError &&
            CLOSED_SESSION_ERROR_CODES.has(error.code)
          ) {
            setSession(null);
            setPreparedUserPrompt(null);
            setRestoredSessionUntouched(false);
            setCommitResult(null);
            setResultFile(null);
            setResultError(null);
            resetPromptCopySequence();
          }
          announcementRef.current(
            `이전 웹 전달을 확인하지 못했습니다. ${errorMessage(error)}`,
            "error",
          );
        }
      })
      .finally(() => {
        if (active) setIsRestoring(false);
      });
    return () => {
      active = false;
    };
  }, [onRestoreLatest, resetPromptCopySequence]);

  const copyText = async (
    value: string,
    fallbackRef: RefObject<HTMLTextAreaElement | null>,
  ) => {
    return copyAiHandoffPrompt(value, {
      clipboardWriteText:
        typeof navigator !== "undefined" && navigator.clipboard?.writeText
          ? (text) => navigator.clipboard.writeText(text)
          : undefined,
      fallbackCopy: () => {
        const input = fallbackRef.current;
        if (!input || typeof document === "undefined") return false;
        input.focus();
        input.select();
        return typeof document.execCommand === "function"
          ? document.execCommand("copy")
          : false;
      },
    });
  };

  const beginAction = (action: WorkingAction) => {
    if (controlsDisabled || !onBusyStart()) {
      onAnnouncement("다른 AI 작업이 끝난 뒤 다시 시도해 주세요.", "error");
      return false;
    }
    setWorkingAction(action);
    return true;
  };

  const finishAction = () => {
    setWorkingAction(null);
    onBusyEnd();
  };

  const copyPreparedPrompt = async (
    preparedSession = session,
    allowDraftMismatch = false,
  ) => {
    if (!preparedSession) return false;
    if (
      !allowDraftMismatch &&
      preparedSession === session &&
      !sessionMatchesDraft
    ) {
      onAnnouncement(
        "현재 웹 서비스·수정 문구와 이전 전달 패키지가 다릅니다. 현재 내용으로 다시 준비해 주세요.",
        "error",
      );
      return false;
    }
    const copyGeneration = ++promptCopyGenerationRef.current;
    const copied = await copyText(preparedSession.finalPrompt, uploadPromptRef);
    if (copyGeneration !== promptCopyGenerationRef.current) return false;
    setPromptCopyState(
      copied === "clipboard"
        ? "copied"
        : copied === "fallback"
          ? "fallback"
          : "failed",
    );
    setPromptCopyRevision((revision) => revision + 1);
    if (copied === "empty" || copied === "failed") {
      onAnnouncement(
        "프롬프트 자동 복사에 실패했습니다. 아래 프롬프트를 직접 복사해 주세요.",
        "error",
      );
      return false;
    }
    onAnnouncement("웹 전달 프롬프트를 클립보드에 복사했습니다.", "status");
    return true;
  };

  const prepare = async () => {
    if (prepareBlocked) return;
    if (hasUnsavedChanges) {
      onAnnouncement(
        "화면과 전달 이미지가 달라지지 않도록 편집 변경을 먼저 적용하거나 되돌려 주세요.",
        "error",
      );
      return;
    }
    if (!beginAction("prepare")) return;
    const requestedPrompt = userPrompt.trim();
    setRestoredSessionUntouched(false);
    setResultFile(null);
    setResultError(null);
    setCommitResult(null);
    setCompletedResult(null);
    resetPromptCopySequence();
    try {
      const prepared = await onPrepare(serviceSurface, requestedPrompt);
      setSession(prepared);
      setPreparedUserPrompt(requestedPrompt);
      const copied = await copyPreparedPrompt(prepared, true);
      if (!copied) {
        return;
      }
      try {
        await onOpenSite(prepared.serviceSurface);
        onAnnouncement(
          "전달 파일과 프롬프트를 준비하고 공식 웹사이트를 열었습니다.",
          "status",
        );
      } catch (error) {
        onAnnouncement(
          `전달 준비는 완료했지만 웹사이트를 열지 못했습니다. ${errorMessage(error)}`,
          "error",
        );
      }
    } catch (error) {
      onAnnouncement(errorMessage(error), "error");
    } finally {
      finishAction();
    }
  };

  const runSessionAction = async (
    action: Extract<WorkingAction, "reveal" | "drag" | "extend" | "delete">,
  ) => {
    if (!session || !beginAction(action)) return;
    try {
      if (action === "reveal") {
        await onRevealUpload(session.requestId);
        onAnnouncement(
          "탐색기에서 업로드 파일을 선택했습니다. 웹 업로드 영역으로 끌어다 놓으세요.",
          "status",
        );
      } else if (action === "drag") {
        const dragResult = await onStartNativeDrag(session.requestId);
        onAnnouncement(dragResult.message, "status");
      } else if (action === "extend") {
        const extended = await onExtendRetention(session.requestId);
        setSession(extended);
        onAnnouncement("웹 전달 보관 기간을 한 번 연장했습니다.", "status");
      } else {
        const deleted = await onDeleteSession(session.requestId);
        setSession(null);
        setPreparedUserPrompt(null);
        setRestoredSessionUntouched(false);
        setCommitResult(null);
        resetPromptCopySequence();
        onAnnouncement(
          deleted.payloadDeleted && !deleted.cleanupDeferred
            ? "이 전달을 닫고 임시 파일을 삭제했습니다."
            : "이 전달을 닫았습니다. 임시 파일은 다음 앱 정리 때 다시 삭제합니다.",
          "status",
        );
      }
    } catch (error) {
      if (
        error instanceof CommandError &&
        CLOSED_SESSION_ERROR_CODES.has(error.code)
      ) {
        setSession(null);
        setPreparedUserPrompt(null);
        setRestoredSessionUntouched(false);
        setCommitResult(null);
        resetPromptCopySequence();
      }
      onAnnouncement(errorMessage(error), "error");
    } finally {
      finishAction();
    }
  };

  const commitFile = async (file: File) => {
    if (!session || !beginAction("commit")) return;
    setResultFile(file);
    setResultError(null);
    setCommitResult(null);
    try {
      const result = await onCommitResult(session.requestId, file);
      setCommitResult(result);
      if (result.accepted) {
        setCompletedResult(result);
        setSession(null);
        setPreparedUserPrompt(null);
        setRestoredSessionUntouched(false);
        resetPromptCopySequence();
        onAnnouncement(
          result.reviewState
            ? "검사를 통과한 결과를 비활성 AI 후보로 보관했습니다. 원본과 현재 적용 이미지는 바뀌지 않았습니다."
            : "결과 후보는 안전하게 저장했습니다. 검토 목록은 AI 검토 영역을 다시 열어 확인해 주세요.",
          "status",
        );
        onCommitted(result);
      } else if (result.issues.some((issue) => issue.severity === "blocking")) {
        onAnnouncement(
          "결과 구조에 문제가 있어 후보로 가져오지 않았습니다. 아래 해결 방법을 확인해 주세요.",
          "error",
        );
      } else {
        onAnnouncement(
          "후보 저장은 끝났지만 검토 목록을 바로 불러오지 못했습니다. AI 검토 영역을 다시 열어 주세요.",
          "status",
        );
      }
    } catch (error) {
      const message = errorMessage(error);
      if (
        error instanceof CommandError &&
        CLOSED_SESSION_ERROR_CODES.has(error.code)
      ) {
        setSession(null);
        setPreparedUserPrompt(null);
        setRestoredSessionUntouched(false);
        setCommitResult(null);
        resetPromptCopySequence();
      }
      setResultError(message);
      onAnnouncement(message, "error");
    } finally {
      finishAction();
    }
  };

  const acceptFiles = (files: Iterable<File> | ArrayLike<File>) => {
    if (!session || controlsDisabled) return;
    const selected = selectAiWebHandoffResultFile(files);
    setResultError(selected.error);
    setCommitResult(null);
    if (!selected.file) {
      setResultFile(null);
      if (selected.error) onAnnouncement(selected.error, "error");
      return;
    }
    void commitFile(selected.file);
  };

  const handleResultDrop = (event: DragEvent<HTMLDivElement>) => {
    event.preventDefault();
    setIsDraggingOver(false);
    acceptFiles(event.dataTransfer.files);
  };

  const handleNativeDragKeyboard = (
    event: KeyboardEvent<HTMLButtonElement>,
  ) => {
    if (event.key !== "Enter" && event.key !== " ") return;
    event.preventDefault();
    void runSessionAction("reveal");
  };

  const handleNativeDragClick = (event: MouseEvent<HTMLButtonElement>) => {
    if (event.detail === 0) {
      void runSessionAction("reveal");
    }
  };

  return (
    <div className="flex flex-col gap-4" data-testid="ai-web-handoff-panel">
      <div className="flex gap-2 rounded-md border border-focus/25 bg-selected/40 p-3 text-xs leading-5">
        <ShieldCheck
          aria-hidden="true"
          className="mt-0.5 size-4 shrink-0 text-focus"
        />
        <p>
          앱이 전달 이미지와 구조 보호 프롬프트를 함께 준비합니다. 로그인과 실제
          업로드·생성·다운로드는 사용자가 웹사이트에서 직접 수행합니다.
        </p>
      </div>

      <div className="grid gap-3 sm:grid-cols-[180px_minmax(0,1fr)]">
        <label className="flex flex-col gap-1 text-xs font-semibold">
          사용할 웹사이트
          <select
            className="min-h-10 rounded-md border border-border bg-white px-3 py-2 text-sm focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
            data-testid="ai-web-handoff-service"
            disabled={controlsDisabled}
            value={serviceSurface}
            onChange={(event) => {
              setServiceSurface(event.currentTarget.value as WebServiceSurface);
              setRestoredSessionUntouched(false);
              setCommitResult(null);
              resetPromptCopySequence();
            }}
          >
            {WEB_SERVICE_OPTIONS.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
        </label>
        <label
          className="flex flex-col gap-1 text-xs font-semibold"
          htmlFor="ai-web-handoff-request"
        >
          {isNovelAi ? "원하는 수정 태그 (영어 권장)" : "원하는 수정"}
          <textarea
            className="min-h-24 resize-y rounded-md border border-border bg-white px-3 py-2 text-sm leading-5 focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
            data-testid="ai-web-handoff-request"
            disabled={controlsDisabled}
            id="ai-web-handoff-request"
            placeholder={
              isNovelAi
                ? "예: same character, brighter smile, chibi, clean lineart"
                : "예: 캐릭터와 구도는 유지하고 표정을 더 밝게 바꿔 주세요."
            }
            value={userPrompt}
            onChange={(event) => {
              setUserPrompt(event.currentTarget.value);
              setRestoredSessionUntouched(false);
              setCommitResult(null);
              resetPromptCopySequence();
            }}
          />
        </label>
      </div>

      {isNovelAi && needsNovelAiEnglishInputHint(userPrompt) ? (
        <p
          className="rounded-md border border-warning/40 bg-warning/10 px-3 py-2 text-xs leading-5 text-warning"
          data-testid="novelai-prompt-language-hint"
        >
          NovelAI V4+에는 영문 소문자 태그를 쉼표로 나눠 입력하는 방식을 권장합니다.
          입력한 한국어는 앱이 임의로 번역하지 않으니, 가능하면 짧은 영문 태그로
          바꿔 주세요.
        </p>
      ) : null}

      {isNovelAi ? (
        <p
          className="rounded-md border border-violet-200 bg-violet-50/70 px-3 py-2 text-xs leading-5 text-violet-950"
          data-testid="novelai-copy-order-hint"
        >
          아래 준비 버튼이 <strong>1/2 Prompt</strong>를 복사하고 NovelAI를
          엽니다. Prompt에 붙여넣은 뒤 이어서 표시되는 안내의{" "}
          <strong>2/2 Undesired Content</strong>를 복사하세요.
        </p>
      ) : null}

      {hasUnsavedChanges ? (
        <p className="rounded-md border border-warning/40 bg-warning/10 px-3 py-2 text-xs font-medium leading-5 text-warning">
          저장하지 않은 편집이 있습니다. 전달 이미지와 화면이 달라지지 않도록
          먼저 적용하거나 되돌려 주세요.
        </p>
      ) : null}

      {isRestoring ? (
        <p className="text-xs text-muted" role="status">
          이전 웹 전달 확인 중…
        </p>
      ) : null}

      <button
        aria-busy={workingAction === "prepare"}
        className="inline-flex min-h-11 items-center justify-center gap-2 rounded-md bg-accent px-4 py-2 text-sm font-semibold text-accent-foreground hover:bg-accent-strong focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:opacity-50"
        data-testid="ai-web-handoff-prepare"
        disabled={prepareBlocked || hasUnsavedChanges}
        type="button"
        onClick={() => {
          void prepare();
        }}
      >
        {workingAction === "prepare" ? (
          <LoaderCircle
            aria-hidden="true"
            className="size-4 animate-spin motion-reduce:animate-none"
          />
        ) : (
          <ImagePlus aria-hidden="true" className="size-4" />
        )}
        {workingAction === "prepare" ? "전달 준비 중" : "웹 AI로 바로 준비"}
      </button>

      {isNovelAi ? (
        <NovelAiWebGuide
          allowsProportionalNormalization
          disabled={controlsDisabled}
          expectedCanvas={expectedCanvasLabel}
          promptCopyOutcome={
            promptCopyState === "copied" || promptCopyState === "fallback"
              ? "copied"
              : promptCopyState === "failed"
                ? "failed"
                : "idle"
          }
          promptCopyRevision={promptCopyRevision}
          task="single_edit"
        />
      ) : null}

      {session && !sessionMatchesDraft ? (
        <p
          className="rounded-md border border-warning/40 bg-warning/10 px-3 py-2 text-xs font-medium leading-5 text-warning"
          data-testid="ai-web-handoff-draft-changed"
        >
          아래 전달 패키지는 이전 웹사이트·수정 문구 기준입니다. 그대로 이어서 쓰거나,
          현재 문구로 다시 준비하면 이전 패키지는 자동으로 닫힙니다.
        </p>
      ) : null}

      {completedResult?.accepted ? (
        <section
          className="rounded-md border border-success/30 bg-success/5 p-3 text-xs leading-5"
          data-testid="ai-web-handoff-completed"
          role="status"
        >
          <p className="flex items-center gap-2 font-semibold text-success">
            <ShieldCheck aria-hidden="true" className="size-4" />
            결과를 비활성 AI 후보로 저장했습니다
          </p>
          <p className="mt-1 text-muted">
            원본과 현재 적용 이미지는 그대로입니다. AI 후보 비교에서 내용을 확인한 뒤
            직접 적용하세요.
          </p>
          {completedResult.issues.some((issue) => issue.severity === "warning") ? (
            <ul className="mt-2 text-warning">
              {completedResult.issues
                .filter((issue) => issue.severity === "warning")
                .map((issue) => (
                  <li key={issue.code}>• {issue.message}</li>
                ))}
            </ul>
          ) : null}
        </section>
      ) : null}

      {session ? (
        <section
          aria-labelledby="ai-web-transfer-ready-title"
          className="flex flex-col gap-3 rounded-lg border border-border bg-preview p-4"
          data-testid="ai-web-handoff-ready"
        >
          <div className="flex flex-wrap items-start justify-between gap-2">
            <div>
              <h4
                className="text-sm font-semibold text-success"
                id="ai-web-transfer-ready-title"
              >
                전달 준비 완료
              </h4>
              <p className="mt-1 text-xs leading-5 text-muted">
                {session.uploadFileName} ·{" "}
                단일 아이콘
              </p>
            </div>
            <span
              className={cn(
                "rounded-full px-2 py-1 text-[11px] font-semibold",
                promptCopyState === "copied" || promptCopyState === "fallback"
                  ? "bg-success/10 text-success"
                  : "bg-warning/10 text-warning",
              )}
              data-testid="ai-web-handoff-copy-status"
            >
              {promptCopyState === "copied"
                ? "프롬프트 복사됨"
                : promptCopyState === "fallback"
                  ? "선택 영역으로 복사됨"
                  : "프롬프트 직접 복사 필요"}
            </span>
          </div>

          <div className="grid gap-3 sm:grid-cols-[144px_minmax(0,1fr)]">
            <div className="flex min-h-36 items-center justify-center overflow-hidden rounded-md border border-border bg-checkerboard">
              <img
                alt={`${session.uploadFileName} 웹 전달 미리보기`}
                className="max-h-40 max-w-full object-contain"
                src={session.uploadPreviewPath}
              />
            </div>
            <div className="flex flex-col gap-2">
              <p className="text-xs font-semibold">빠른 왕복</p>
              <ol className="grid gap-1 text-xs leading-5 text-muted">
                <li>1. 아래 파일을 웹 업로드 영역에 놓기</li>
                <li>2. 복사된 프롬프트 붙여넣기</li>
                {session.serviceSurface === "novelai_web" ? (
                  <li>
                    3. 앱으로 돌아와 제외 태그를 복사하고 Undesired Content에
                    붙여넣기
                  </li>
                ) : null}
                <li>
                  {session.serviceSurface === "novelai_web" ? "4" : "3"}.
                  Download Image로 PNG·JPG·WebP 저장
                </li>
                <li>
                  {session.serviceSurface === "novelai_web" ? "5" : "4"}.
                  내려받은 결과를 아래 영역에 놓기
                </li>
              </ol>
              <div className="flex flex-wrap gap-2">
                {session.nativeDragSupported ? (
                  <button
                    aria-describedby="ai-native-drag-keyboard-help"
                    className="inline-flex min-h-9 items-center gap-2 rounded-md bg-accent px-3 py-2 text-xs font-semibold text-accent-foreground hover:bg-accent-strong focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:opacity-50"
                    data-testid="ai-web-handoff-native-drag"
                    disabled={controlsDisabled}
                    type="button"
                    onClick={handleNativeDragClick}
                    onKeyDown={handleNativeDragKeyboard}
                    onPointerDown={(event) => {
                      if (event.pointerType === "mouse" && event.button === 0) {
                        void runSessionAction("drag");
                      }
                    }}
                  >
                    <GripVertical aria-hidden="true" className="size-4" />이
                    파일 끌기
                  </button>
                ) : null}
                <button
                  className="inline-flex min-h-9 items-center gap-2 rounded-md border border-border bg-white px-3 py-2 text-xs font-semibold hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:opacity-50"
                  data-testid="ai-web-handoff-reveal"
                  disabled={controlsDisabled}
                  type="button"
                  onClick={() => {
                    void runSessionAction("reveal");
                  }}
                >
                  <FolderOpen aria-hidden="true" className="size-4" />
                  탐색기에서 파일 선택
                </button>
              </div>
              <p
                className="text-[11px] leading-4 text-muted"
                id="ai-native-drag-keyboard-help"
              >
                키보드 사용자는 탐색기에서 파일 선택을 누른 뒤 복사·붙여넣기
                또는 탐색기 키보드 조작을 사용할 수 있습니다.
              </p>
            </div>
          </div>

          <div className="flex flex-wrap gap-2">
            <button
              className="inline-flex min-h-9 items-center gap-2 rounded-md border border-border bg-white px-3 py-2 text-xs font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:opacity-50"
              disabled={controlsDisabled || !sessionMatchesDraft}
              type="button"
              onClick={() => {
                void copyPreparedPrompt();
              }}
            >
              <Copy aria-hidden="true" className="size-3.5" />
              프롬프트 다시 복사
            </button>
            <button
              className="inline-flex min-h-9 items-center gap-2 rounded-md border border-border bg-white px-3 py-2 text-xs font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:opacity-50"
              disabled={controlsDisabled}
              type="button"
              onClick={() => {
                void onOpenSite(session.serviceSurface).catch((error) =>
                  onAnnouncement(errorMessage(error), "error"),
                );
              }}
            >
              <ExternalLink aria-hidden="true" className="size-3.5" />
              공식 웹 다시 열기
            </button>
            {session.canExtend ? (
              <button
                className="inline-flex min-h-9 items-center gap-2 rounded-md border border-border bg-white px-3 py-2 text-xs font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:opacity-50"
                disabled={controlsDisabled}
                type="button"
                onClick={() => {
                  void runSessionAction("extend");
                }}
              >
                <Clock3 aria-hidden="true" className="size-3.5" />
                보관 기간 한 번 연장
              </button>
            ) : null}
            <button
              className="inline-flex min-h-9 items-center gap-2 rounded-md border border-border bg-white px-3 py-2 text-xs font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:opacity-50"
              data-testid="ai-web-handoff-delete"
              disabled={controlsDisabled}
              type="button"
              onClick={() => {
                void runSessionAction("delete");
              }}
            >
              <Trash2 aria-hidden="true" className="size-3.5" />
              이 전달 닫기
            </button>
          </div>

          <label
            className="flex flex-col gap-1 text-xs font-semibold"
            htmlFor="ai-web-handoff-final-prompt"
          >
            {session.serviceSurface === "novelai_web"
              ? "NovelAI Prompt (태그)"
              : "준비된 최종 프롬프트"}
            <textarea
              className="min-h-28 resize-y rounded-md border border-border bg-white px-3 py-2 text-xs leading-5"
              id="ai-web-handoff-final-prompt"
              readOnly
              ref={uploadPromptRef}
              value={session.finalPrompt}
            />
          </label>

          <div
            className={cn(
              "rounded-lg border-2 border-dashed p-4 transition-colors motion-reduce:transition-none",
              isDraggingOver
                ? "border-focus bg-selected"
                : "border-border bg-white",
            )}
            data-testid="ai-web-handoff-result-drop"
            onDragEnter={(event) => {
              event.preventDefault();
              setIsDraggingOver(true);
            }}
            onDragLeave={(event) => {
              if (event.currentTarget === event.target) {
                setIsDraggingOver(false);
              }
            }}
            onDragOver={(event) => {
              event.preventDefault();
              event.dataTransfer.dropEffect = "copy";
            }}
            onDrop={handleResultDrop}
          >
            <div className="flex flex-col items-center gap-2 text-center">
              {workingAction === "commit" ? (
                <LoaderCircle
                  aria-hidden="true"
                  className="size-6 animate-spin text-focus motion-reduce:animate-none"
                />
              ) : (
                <ImagePlus aria-hidden="true" className="size-6 text-focus" />
              )}
              <p className="text-sm font-semibold">
                {workingAction === "commit"
                  ? "결과 구조 검사 중"
                  : "내려받은 PNG·JPG·WebP를 여기에 놓으세요"}
              </p>
              <p className="text-xs leading-5 text-muted">
                웹페이지 미리보기 주소가 아닌 Download Image로 저장한 파일 한 장을 사용합니다. WebP는 내부 PNG로 안전하게 변환합니다.
              </p>
              <input
                accept={AI_WEB_HANDOFF_RESULT_ACCEPT}
                className="sr-only"
                data-testid="ai-web-handoff-result-input"
                disabled={controlsDisabled}
                ref={resultInputRef}
                type="file"
                onChange={(event) => {
                  acceptFiles(event.currentTarget.files ?? []);
                  event.currentTarget.value = "";
                }}
              />
              <button
                className="min-h-9 rounded-md border border-border bg-white px-3 py-2 text-xs font-semibold hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:opacity-50"
                disabled={controlsDisabled}
                type="button"
                onClick={() => resultInputRef.current?.click()}
              >
                결과 파일 선택
              </button>
              {resultFile ? (
                <p className="max-w-full truncate text-xs text-muted">
                  검사한 파일: {resultFile.name}
                </p>
              ) : null}
            </div>
          </div>

          {resultError ? (
            <p
              className="rounded-md border border-danger/30 bg-danger/5 px-3 py-2 text-xs font-medium leading-5 text-danger"
              role="alert"
            >
              {resultError}
            </p>
          ) : null}

          {commitResult?.issues.length ? (
            <section
              aria-labelledby="ai-web-handoff-issues-title"
              className="flex flex-col gap-3"
              data-testid="ai-web-handoff-issues"
            >
              <div className="flex items-center gap-2">
                <AlertTriangle
                  aria-hidden="true"
                  className="size-4 text-warning"
                />
                <h5
                  className="text-sm font-semibold"
                  id="ai-web-handoff-issues-title"
                >
                  결과 검사 안내
                </h5>
              </div>
              {commitResult.issues.map((issue, index) => {
                const guidance = describeAiWebHandoffIssue(issue);
                return (
                  <article
                    className="rounded-md border border-border bg-white p-3 text-xs leading-5"
                    key={`${issue.code}-${index}`}
                  >
                    <p
                      className={
                        issue.severity === "blocking"
                          ? "font-semibold text-danger"
                          : issue.severity === "warning"
                            ? "font-semibold text-warning"
                            : "font-semibold text-foreground"
                      }
                    >
                      {issue.severity === "blocking"
                        ? "문제"
                        : issue.severity === "warning"
                          ? "경고"
                          : "확인"}
                      : {guidance.problem}
                    </p>
                    <p className="mt-1 text-muted">영향: {guidance.impact}</p>
                    {issue.expected || issue.actual ? (
                      <dl className="mt-2 grid grid-cols-[72px_minmax(0,1fr)] gap-x-2 rounded bg-preview px-2 py-1">
                        <dt className="font-medium text-muted">예상</dt>
                        <dd>{issue.expected ?? "-"}</dd>
                        <dt className="font-medium text-muted">실제</dt>
                        <dd>{issue.actual ?? "-"}</dd>
                      </dl>
                    ) : null}
                    <p className="mt-2 font-medium">해결: {guidance.fix}</p>
                    {guidance.correctionPrompt ? (
                      <p className="mt-1 text-focus">
                        추가 문장: {guidance.correctionPrompt}
                      </p>
                    ) : (
                      <p className="mt-1 text-muted">
                        이 문제는 프롬프트 추가보다 위 조치가 먼저 필요합니다.
                      </p>
                    )}
                  </article>
                );
              })}
              {correctionPrompt ? (
                <div className="flex flex-col gap-2 rounded-md border border-focus/25 bg-selected/30 p-3">
                  <label
                    className="text-xs font-semibold"
                    htmlFor="ai-web-handoff-correction-prompt"
                  >
                    다음 요청에 추가할 구조 수정 문장
                  </label>
                  <textarea
                    className="min-h-24 resize-y rounded-md border border-border bg-white px-3 py-2 text-xs leading-5"
                    id="ai-web-handoff-correction-prompt"
                    readOnly
                    ref={correctionPromptRef}
                    value={correctionPrompt}
                  />
                  <button
                    className="inline-flex min-h-9 items-center justify-center gap-2 rounded-md border border-border bg-white px-3 py-2 text-xs font-semibold hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
                    type="button"
                    onClick={() => {
                      void copyText(correctionPrompt, correctionPromptRef).then(
                        (result) =>
                          onAnnouncement(
                            result === "clipboard" || result === "fallback"
                              ? "구조 수정 문장을 복사했습니다."
                              : "복사하지 못했습니다. 문장을 직접 선택해 주세요.",
                            result === "clipboard" || result === "fallback"
                              ? "status"
                              : "error",
                          ),
                      );
                    }}
                  >
                    <Copy aria-hidden="true" className="size-4" />
                    구조 수정 문장 복사
                  </button>
                </div>
              ) : null}
            </section>
          ) : null}

          {session.warnings.length ? (
            <ul className="rounded-md border border-warning/30 bg-warning/5 px-4 py-2 text-xs leading-5 text-warning">
              {session.warnings.map((warning) => (
                <li key={warning}>• {warning}</li>
              ))}
            </ul>
          ) : null}
        </section>
      ) : null}

      <details className="rounded-md border border-border bg-white p-3">
        <summary className="cursor-pointer text-xs font-semibold">
          웹사이트에서 오류가 표시됐나요?
        </summary>
        <div className="mt-3 flex flex-col gap-2">
          <label
            className="text-xs font-medium"
            htmlFor="ai-web-handoff-web-error"
          >
            웹 오류 문구 붙여넣기
          </label>
          <textarea
            className="min-h-20 resize-y rounded-md border border-border bg-white px-3 py-2 text-xs leading-5 focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
            id="ai-web-handoff-web-error"
            placeholder="예: 429 quota exceeded"
            value={webErrorText}
            onChange={(event) => setWebErrorText(event.currentTarget.value)}
          />
          {pastedWebErrorGuidance ? (
            <div
              className="rounded-md border border-border bg-preview p-3 text-xs leading-5"
              data-testid="ai-web-handoff-web-error-guidance"
              role="status"
            >
              <p className="font-semibold">{pastedWebErrorGuidance.title}</p>
              <p className="mt-1 text-muted">{pastedWebErrorGuidance.action}</p>
              <p className="mt-1 font-medium">
                이 오류에는 임의의 추가 프롬프트를 만들지 않습니다.
              </p>
            </div>
          ) : null}
        </div>
      </details>
    </div>
  );
}
