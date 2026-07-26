import { Wand2 } from "lucide-react";

import type {
  AutoDetectSheetGridProposal,
  AutoDetectSheetGridResult,
} from "@/features/sheets/types";
import { cn } from "@/lib/utils";

export function SheetAutoDetectPanel({
  file,
  result,
  isRunning,
  errorMessage,
  onRun,
  onApplyProposal,
}: {
  file: File | null;
  result: AutoDetectSheetGridResult | null;
  isRunning: boolean;
  errorMessage: string | null;
  onRun: () => void;
  onApplyProposal: (proposal: AutoDetectSheetGridProposal) => void;
}) {
  return (
    <section className="flex flex-col gap-4 rounded-md border border-border bg-white p-4">
      <div className="flex flex-wrap items-start justify-between gap-3">
        <div>
          <h3 className="flex items-center gap-2 text-sm font-semibold">
            <Wand2 className="size-4" aria-hidden="true" />
            자동 감지 제안
          </h3>
          <p className="mt-1 max-w-2xl text-sm text-muted">
            투명 separator 또는 단색 배경 separator를 찾아 grid 설정 후보를 만듭니다.
            제안은 자동 가져오기를 실행하지 않으며, 적용 후 overlay에서 검토해야 합니다.
          </p>
        </div>
        <button
          className="rounded-md bg-accent px-3 py-2 text-sm font-semibold text-accent-foreground hover:bg-accent-strong disabled:opacity-60"
          disabled={!file || isRunning}
          type="button"
          onClick={onRun}
        >
          {isRunning ? "감지 중" : "자동 감지 실행"}
        </button>
      </div>

      {file ? (
        <div className="rounded-md border border-border bg-preview px-3 py-2 text-xs text-muted">
          대상 시트: <span className="font-medium text-foreground">{file.name}</span>
        </div>
      ) : (
        <div className="rounded-md border border-border bg-preview px-3 py-2 text-sm text-muted">
          먼저 PNG/JPG/JPEG 시트를 선택하세요.
        </div>
      )}

      {result ? (
        <div className="grid gap-3">
          <div className="text-xs text-muted">
            {result.sheetWidth}x{result.sheetHeight} · alpha{" "}
            {result.hasAlpha ? "있음" : "없음"} · 제안 {result.proposals.length}개
          </div>
          {result.warnings.length ? (
            <div className="rounded-md border border-border bg-preview p-3 text-sm text-muted">
              {result.warnings.join(" / ")}
            </div>
          ) : null}
          {result.proposals.length ? (
            <div className="grid gap-3 xl:grid-cols-2">
              {result.proposals.map((proposal) => (
                <AutoDetectProposalCard
                  disabled={isRunning}
                  key={proposal.id}
                  proposal={proposal}
                  onApply={() => onApplyProposal(proposal)}
                />
              ))}
            </div>
          ) : (
            <div className="rounded-md border border-border bg-preview p-3 text-sm text-muted">
              신뢰할 수 있는 후보가 없습니다. Grid, 셀 크기, 직접 Slice를 사용하세요.
            </div>
          )}
        </div>
      ) : null}

      {errorMessage ? (
        <p className="text-sm text-danger" role="alert">
          {errorMessage}
        </p>
      ) : null}
    </section>
  );
}

function AutoDetectProposalCard({
  disabled = false,
  proposal,
  onApply,
}: {
  disabled?: boolean;
  proposal: AutoDetectSheetGridProposal;
  onApply: () => void;
}) {
  const settings = proposal.gridSettings;
  return (
    <article className="rounded-md border border-border bg-surface p-3">
      <div className="flex items-start justify-between gap-3">
        <div>
          <h4 className="text-sm font-semibold">{proposal.label}</h4>
          <p className="mt-1 text-xs text-muted">
            {proposal.method} · confidence {proposal.confidence} ·{" "}
            {Math.round(proposal.confidenceScore * 100)}%
          </p>
        </div>
        <span
          className={cn(
            "rounded px-2 py-1 text-xs font-semibold",
            proposal.confidence === "high"
              ? "bg-selected text-accent"
              : proposal.confidence === "medium"
                ? "bg-preview text-foreground"
                : "bg-danger/10 text-danger",
          )}
        >
          {proposal.confidence}
        </span>
      </div>
      <dl className="mt-3 grid grid-cols-2 gap-2 text-xs">
        <GridSpec label="행/열" value={`${proposal.computedRows} / ${proposal.computedColumns}`} />
        <GridSpec label="셀" value={`${settings.cellWidth}x${settings.cellHeight}`} />
        <GridSpec label="간격" value={`${settings.gapX}/${settings.gapY}`} />
        <GridSpec
          label="여백"
          value={`${settings.borderLeft}/${settings.borderTop}/${settings.borderRight}/${settings.borderBottom}`}
        />
      </dl>
      {proposal.warnings.length ? (
        <p className="mt-3 text-xs text-muted">{proposal.warnings.join(" / ")}</p>
      ) : null}
      <button
        className="mt-3 rounded-md border border-border bg-white px-3 py-2 text-sm font-medium hover:bg-menu-hover disabled:cursor-not-allowed disabled:text-muted"
        disabled={disabled}
        type="button"
        onClick={onApply}
      >
        이 제안 적용 후 overlay 확인
      </button>
    </article>
  );
}

function GridSpec({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <dt className="text-muted">{label}</dt>
      <dd className="font-medium text-foreground">{value}</dd>
    </div>
  );
}
