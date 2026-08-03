import { AlertTriangle, Copy, Sparkles } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import {
  buildNovelAiGuideSpec,
  novelAiUndesiredContentForTask,
  type NovelAiBackgroundPolicy,
  type NovelAiWebTask,
} from "@/features/ai-web/novelai-web-model";
import { copyAiHandoffPrompt } from "@/features/editor/ai-provider-model";

export type NovelAiPromptCopyOutcome = "idle" | "copied" | "failed";

export function NovelAiWebGuide({
  task,
  expectedCanvas,
  hasReference = false,
  allowsProportionalNormalization = false,
  backgroundPolicy = "preserve_transparency",
  disabled = false,
  promptCopyOutcome,
  promptCopyRevision = 0,
}: {
  task: NovelAiWebTask;
  expectedCanvas: string;
  hasReference?: boolean;
  allowsProportionalNormalization?: boolean;
  backgroundPolicy?: NovelAiBackgroundPolicy;
  disabled?: boolean;
  promptCopyOutcome?: NovelAiPromptCopyOutcome;
  promptCopyRevision?: number;
}) {
  const undesiredRef = useRef<HTMLTextAreaElement>(null);
  const copyGenerationRef = useRef(0);
  const [copyState, setCopyState] = useState<
    | "prompt_first"
    | "prompt_copied"
    | "undesired_copied"
    | "prompt_copy_failed"
    | "undesired_copy_failed"
  >("prompt_first");
  const spec = buildNovelAiGuideSpec({
    task,
    expectedCanvas,
    hasReference,
    allowsProportionalNormalization,
    backgroundPolicy,
  });
  const undesiredContent = novelAiUndesiredContentForTask(
    task,
    backgroundPolicy,
  );
  const managedPromptSequence = promptCopyOutcome !== undefined;

  useEffect(() => {
    if (!managedPromptSequence) return;
    copyGenerationRef.current += 1;
    setCopyState(
      promptCopyOutcome === "copied"
        ? "prompt_copied"
        : promptCopyOutcome === "failed"
          ? "prompt_copy_failed"
          : "prompt_first",
    );
  }, [managedPromptSequence, promptCopyOutcome, promptCopyRevision, task]);

  const copyStateMessage =
    copyState === "prompt_first"
      ? "현재 1/2: 먼저 NovelAI Prompt를 복사해 Prompt 입력란에 붙여 넣으세요. 그러면 2단계 버튼이 열립니다."
      : copyState === "prompt_copied"
        ? "1/2 완료: Prompt가 복사되었습니다. NovelAI Prompt 입력란에 붙여 넣은 뒤 2/2 제외 태그를 복사하세요."
        : copyState === "undesired_copied"
          ? "2/2 완료: Undesired Content가 복사되었습니다. NovelAI의 Undesired Content 입력란에 붙여 넣으세요."
          : copyState === "prompt_copy_failed"
            ? "1단계 Prompt 자동 복사에 실패했습니다. Prompt를 직접 복사·붙여넣은 뒤 2단계 제외 태그를 복사하세요."
            : "2단계 자동 복사에 실패했습니다. 아래 제외 태그를 직접 복사해 Undesired Content 입력란에 붙여 넣으세요.";
  const hasCopyFailure =
    copyState === "prompt_copy_failed" || copyState === "undesired_copy_failed";

  const copyUndesired = async () => {
    const copyGeneration = ++copyGenerationRef.current;
    const result = await copyAiHandoffPrompt(undesiredContent, {
      clipboardWriteText:
        typeof navigator !== "undefined" && navigator.clipboard?.writeText
          ? (value) => navigator.clipboard.writeText(value)
          : undefined,
      fallbackCopy: () => {
        const input = undesiredRef.current;
        if (!input || typeof document === "undefined") return false;
        input.focus();
        input.select();
        return typeof document.execCommand === "function"
          ? document.execCommand("copy")
          : false;
      },
    });
    if (copyGeneration !== copyGenerationRef.current) return;
    setCopyState(
      result === "clipboard" || result === "fallback"
        ? "undesired_copied"
        : "undesired_copy_failed",
    );
  };

  return (
    <section
      className="rounded-md border border-violet-300/60 bg-violet-50/70 p-3 text-xs leading-5"
      data-testid={`novelai-web-guide-${task}`}
    >
      <div className="flex items-start gap-2">
        <Sparkles aria-hidden="true" className="mt-0.5 size-4 shrink-0 text-violet-700" />
        <div>
          <h4 className="font-semibold text-violet-950">NovelAI 사용 순서</h4>
          <p className="mt-1 text-violet-950">
            업로드 방식: <strong>{spec.recommendedMode}</strong>
          </p>
          <p className="text-violet-900/80">{spec.modeReason}</p>
        </div>
      </div>
      <ol className="mt-2 list-decimal space-y-1 pl-5 text-violet-950/85">
        {spec.steps.map((step) => <li key={step}>{step}</li>)}
      </ol>
      <p className="mt-2 rounded border border-violet-200 bg-white/80 px-2 py-1.5 text-violet-950">
        {spec.resolutionText}
      </p>
      {spec.warningText ? (
        <p className="mt-2 flex items-start gap-1.5 text-violet-900/85">
          <AlertTriangle aria-hidden="true" className="mt-0.5 size-3.5 shrink-0" />
          <span>{spec.warningText}</span>
        </p>
      ) : null}
      <div
        className="mt-3 rounded border border-violet-200 bg-white/80 p-2 text-violet-950"
        data-testid={`novelai-copy-sequence-${task}`}
      >
        <p className="font-semibold">복사 순서: Prompt → Undesired Content</p>
        <ol className="mt-1 list-decimal space-y-1 pl-5 text-violet-900/85">
          <li>
            NovelAI Prompt 복사 버튼으로 Prompt를 복사해 NovelAI의 Prompt 입력란에 붙여
            넣습니다.
          </li>
          <li>
            아래 버튼으로 제외 태그를 복사해 Undesired Content 입력란에
            붙여 넣습니다. 화면에 따라 Prompt와 같은 카드의 탭 또는 아래쪽 별도 필드로 보일 수 있습니다.
          </li>
        </ol>
        <p
          className={
            hasCopyFailure
              ? "mt-2 font-semibold text-danger"
              : "mt-2 font-semibold"
          }
          data-testid={`novelai-copy-state-${task}`}
          role={hasCopyFailure ? "alert" : "status"}
        >
          {copyStateMessage}
        </p>
      </div>
      <label className="mt-3 block font-semibold text-violet-950">
        Undesired Content
        <textarea
          className="mt-1 min-h-20 w-full resize-y rounded border border-violet-200 bg-white p-2 font-mono text-[11px] leading-4 text-foreground"
          data-testid={`novelai-undesired-${task}`}
          readOnly
          ref={undesiredRef}
          value={undesiredContent}
        />
      </label>
      <button
        className="mt-2 inline-flex min-h-8 items-center gap-1.5 rounded border border-violet-300 bg-white px-2.5 py-1.5 font-semibold text-violet-950 hover:bg-violet-100 disabled:opacity-50"
        data-testid={`novelai-copy-undesired-${task}`}
        disabled={disabled || (managedPromptSequence && promptCopyOutcome === "idle")}
        type="button"
        onClick={() => void copyUndesired()}
      >
        <Copy aria-hidden="true" className="size-3.5" />
        2. Undesired Content 복사
      </button>
    </section>
  );
}
