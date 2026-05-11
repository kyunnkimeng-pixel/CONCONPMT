import { AlertTriangle, CheckCircle2, Info } from "lucide-react";

import type { ExportValidationResult } from "@/features/export/types";
import { cn } from "@/lib/utils";

interface ValidationResultListProps {
  result: ExportValidationResult | null;
}

export function ValidationResultList({ result }: ValidationResultListProps) {
  if (!result) {
    return null;
  }

  const hasIssues = result.errors.length > 0 || result.warnings.length > 0;

  return (
    <section className="flex flex-col gap-3 border-t border-border pt-4">
      <div className="flex items-center justify-between gap-3">
        <h3 className="text-sm font-semibold tracking-normal">검증 결과</h3>
        <span
          className={cn(
            "inline-flex items-center gap-1 rounded px-2 py-1 text-xs font-medium",
            result.canExport
              ? "bg-selected text-foreground"
              : "bg-red-50 text-danger",
          )}
        >
          {result.canExport ? (
            <CheckCircle2 aria-hidden="true" className="size-4" />
          ) : (
            <AlertTriangle aria-hidden="true" className="size-4" />
          )}
          {result.outputCount}개 출력
        </span>
      </div>

      {hasIssues ? (
        <div className="flex flex-col gap-2">
          {result.errors.map((issue) => (
            <IssueRow
              issueMessage={issue.message}
              key={`${issue.code}-${issue.pieceId}`}
              suffix={issue.blocking ? "차단" : "내보냄"}
            />
          ))}
          {result.warnings.map((issue) => (
            <IssueRow
              isWarning
              issueMessage={issue.message}
              key={`${issue.code}-${issue.iconId}-${issue.pieceId}`}
            />
          ))}
        </div>
      ) : (
        <p className="inline-flex items-center gap-2 rounded-md border border-border bg-white px-3 py-2 text-sm text-muted">
          <CheckCircle2 aria-hidden="true" className="size-4 text-focus" />
          차단 오류가 없습니다.
        </p>
      )}

      {result.items.length > 0 ? (
        <div className="max-h-48 overflow-auto rounded-md border border-border">
          <table className="w-full border-collapse text-left text-xs">
            <thead className="sticky top-0 bg-surface text-muted">
              <tr>
                <th className="border-b border-border px-3 py-2 font-medium">순서</th>
                <th className="border-b border-border px-3 py-2 font-medium">파일</th>
                <th className="border-b border-border px-3 py-2 font-medium">alt</th>
                <th className="border-b border-border px-3 py-2 font-medium">크기</th>
              </tr>
            </thead>
            <tbody>
              {result.items.map((item) => (
                <tr className="odd:bg-canvas" key={item.pieceId}>
                  <td className="px-3 py-2 tabular-nums">
                    {item.exportIndex.toString().padStart(3, "0")}
                  </td>
                  <td className="max-w-[180px] truncate px-3 py-2">{item.fileName}</td>
                  <td className="px-3 py-2">{item.altText || "-"}</td>
                  <td className="whitespace-nowrap px-3 py-2">
                    {item.width}×{item.height}
                    {item.byteSize ? ` · ${formatBytes(item.byteSize)}` : ""}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : null}
    </section>
  );
}

function IssueRow({
  issueMessage,
  isWarning = false,
  suffix,
}: {
  issueMessage: string;
  isWarning?: boolean;
  suffix?: string;
}) {
  return (
    <p
      className={cn(
        "flex items-start gap-2 rounded-md border px-3 py-2 text-sm",
        isWarning ? "border-amber-200 bg-amber-50 text-amber-900" : "border-red-200 bg-red-50 text-danger",
      )}
    >
      {isWarning ? (
        <Info aria-hidden="true" className="mt-0.5 size-4 shrink-0" />
      ) : (
        <AlertTriangle aria-hidden="true" className="mt-0.5 size-4 shrink-0" />
      )}
      <span className="min-w-0 flex-1">{issueMessage}</span>
      {suffix ? <span className="shrink-0 text-xs font-semibold">{suffix}</span> : null}
    </p>
  );
}

function formatBytes(bytes: number) {
  if (bytes < 1024) {
    return `${bytes} B`;
  }

  if (bytes < 1024 * 1024) {
    return `${(bytes / 1024).toFixed(1)} KB`;
  }

  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
