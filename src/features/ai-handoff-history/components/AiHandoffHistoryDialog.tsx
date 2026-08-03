import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Link } from "@tanstack/react-router";
import {
  Clock3,
  FolderOpen,
  GripVertical,
  HardDrive,
  RefreshCw,
  Trash2,
  X,
} from "lucide-react";

import {
  cancelAiGridWorkspace,
  revealAiGridInput,
  startAiGridInputDrag,
} from "@/features/ai-grid/api";
import {
  getAiWebHandoffStorageStatus,
  listRecentAiWebHandoffs,
  runAiWebHandoffMaintenance,
} from "@/features/ai-handoff-history/api";
import type {
  AiWebHandoffHistoryItem,
  AiWebHandoffStorageStatus,
} from "@/features/ai-handoff-history/types";
import {
  deleteAiWebHandoffPayload,
  revealAiWebHandoffUpload,
  startAiWebHandoffDrag,
} from "@/features/editor/api";
import { getCommandErrorMessage } from "@/lib/tauri";
import { useModalFocus } from "@/lib/use-modal-focus";

export function AiHandoffHistoryDialog({ onClose }: { onClose: () => void }) {
  const dialogRef = useRef<HTMLDivElement>(null);
  useModalFocus(dialogRef, onClose);
  const [items, setItems] = useState<AiWebHandoffHistoryItem[]>([]);
  const [storage, setStorage] = useState<AiWebHandoffStorageStatus | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [workingRequestId, setWorkingRequestId] = useState<string | null>(null);
  const [isMaintaining, setIsMaintaining] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const reload = useCallback(async () => {
    const [nextItems, nextStorage] = await Promise.all([
      listRecentAiWebHandoffs(),
      getAiWebHandoffStorageStatus(),
    ]);
    setItems(nextItems);
    setStorage(nextStorage);
  }, []);

  useEffect(() => {
    let cancelled = false;
    setIsLoading(true);
    void reload()
      .catch((error) => {
        if (!cancelled) setErrorMessage(getCommandErrorMessage(error));
      })
      .finally(() => {
        if (!cancelled) setIsLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [reload]);

  const storagePercent = useMemo(() => {
    if (!storage || storage.quotaBytes <= 0) return 0;
    return Math.min(100, Math.round((storage.usedBytes / storage.quotaBytes) * 100));
  }, [storage]);

  const runItemAction = async (
    item: AiWebHandoffHistoryItem,
    action: "drag" | "reveal" | "delete",
  ) => {
    if (workingRequestId || isMaintaining) return;
    const usesGridInputActions = usesAiGridInputActions(item);
    if (isAiGridHandoff(item) && !usesGridInputActions) return;
    setWorkingRequestId(item.requestId);
    setMessage(null);
    setErrorMessage(null);
    try {
      if (action === "drag") {
        const result = usesGridInputActions
          ? await startAiGridInputDrag(item.requestId)
          : await startAiWebHandoffDrag(item.requestId);
        setMessage(result.message);
      } else if (action === "reveal") {
        if (usesGridInputActions) {
          await revealAiGridInput(item.requestId);
          setMessage("탐색기에서 그리드 입력 파일을 선택했습니다.");
        } else {
          await revealAiWebHandoffUpload(item.requestId);
          setMessage("탐색기에서 업로드 파일을 선택했습니다.");
        }
      } else if (usesGridInputActions) {
        if (
          !window.confirm(
            "이 AI 그리드 요청을 취소할까요? 원본 아이콘과 현재 이미지는 바뀌지 않습니다.",
          )
        ) {
          return;
        }
        await cancelAiGridWorkspace(item.requestId);
        setMessage(
          "AI 그리드 요청을 취소했습니다. 임시 파일 정리는 유지보수에서 처리합니다.",
        );
      } else {
        if (!window.confirm("이 전달을 닫고 보관 중인 임시 파일을 정리할까요?")) return;
        const result = await deleteAiWebHandoffPayload(item.requestId);
        setMessage(
          result.payloadDeleted && !result.cleanupDeferred
            ? "전달을 닫고 임시 파일을 삭제했습니다. 기록은 최근 전달에 남습니다."
            : "전달을 닫았습니다. 파일 정리는 다음 유지보수 때 다시 시도합니다.",
        );
      }
      await reload();
    } catch (error) {
      setErrorMessage(getCommandErrorMessage(error));
      await reload().catch(() => undefined);
    } finally {
      setWorkingRequestId(null);
    }
  };

  const handleMaintenance = async () => {
    if (isMaintaining || workingRequestId) return;
    setIsMaintaining(true);
    setMessage(null);
    setErrorMessage(null);
    try {
      const report = await runAiWebHandoffMaintenance();
      setStorage(report.storage);
      setMessage(
        report.deferredCount > 0
          ? `${report.removedCount}개를 정리했고 ${report.deferredCount}개는 다음 주기에 다시 시도합니다.`
          : `${report.removedCount}개의 만료·닫힌 임시 전달 파일을 정리했습니다.`,
      );
      setItems(await listRecentAiWebHandoffs());
    } catch (error) {
      setErrorMessage(getCommandErrorMessage(error));
    } finally {
      setIsMaintaining(false);
    }
  };

  return (
    <div className="fixed inset-0 z-[80] flex items-center justify-center bg-black/35 p-4" role="presentation">
      <div
        ref={dialogRef}
        aria-labelledby="ai-handoff-history-title"
        aria-modal="true"
        className="flex max-h-[min(760px,calc(100vh-32px))] w-full max-w-4xl flex-col overflow-hidden rounded-xl border border-border bg-surface shadow-2xl"
        data-testid="ai-handoff-history-dialog"
        role="dialog"
        tabIndex={-1}
      >
        <header className="flex items-start justify-between gap-4 border-b border-border px-5 py-4">
          <div>
            <h2 className="text-lg font-semibold" id="ai-handoff-history-title">최근 AI 전달</h2>
            <p className="mt-1 text-sm text-muted">
              로그인 정보는 저장하지 않습니다. 전달 파일은 임시 보관하고 기록만 남깁니다.
            </p>
            <p
              className="mt-1 text-xs text-muted"
              id="ai-handoff-drag-keyboard-help"
            >
              마우스는 파일 끌기를 시작하고, 키보드로 활성화하면 탐색기에서
              파일을 선택합니다.
            </p>
          </div>
          <button
            aria-label="최근 AI 전달 닫기"
            className="rounded-md p-2 text-muted hover:bg-menu-hover hover:text-foreground focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
            type="button"
            onClick={onClose}
          >
            <X aria-hidden="true" />
          </button>
        </header>

        <div className="grid min-h-0 flex-1 gap-4 overflow-y-auto p-5">
          <section className="rounded-lg border border-border bg-canvas p-4" data-testid="ai-handoff-storage-card">
            <div className="flex flex-wrap items-center justify-between gap-3">
              <div className="flex items-center gap-3">
                <HardDrive aria-hidden="true" className="text-accent" />
                <div>
                  <h3 className="text-sm font-semibold">임시 전달 저장 공간</h3>
                  <p className="mt-1 text-xs text-muted">
                    {storage
                      ? `${formatBytes(storage.usedBytes)} / ${formatBytes(storage.quotaBytes)} · 진행 중 ${storage.livePayloadCount}개 · 정리 대기 ${storage.cleanupPendingCount}개 · 기록 ${storage.retainedHistoryCount}개`
                      : "저장 공간을 확인하는 중입니다."}
                  </p>
                </div>
              </div>
              <button
                className="inline-flex min-h-9 items-center gap-2 rounded-md border border-border bg-white px-3 py-2 text-xs font-semibold hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:opacity-50"
                disabled={isMaintaining || Boolean(workingRequestId)}
                type="button"
                onClick={() => void handleMaintenance()}
              >
                <RefreshCw aria-hidden="true" className={isMaintaining ? "animate-spin" : undefined} />
                지금 정리
              </button>
            </div>
            <div
              aria-label={`임시 전달 저장 공간 ${storagePercent}% 사용`}
              className="mt-3 h-2 overflow-hidden rounded-full bg-border"
              role="progressbar"
              aria-valuemin={0}
              aria-valuemax={100}
              aria-valuenow={storagePercent}
            >
              <div
                className={storage?.quotaReached ? "h-full bg-danger" : "h-full bg-accent"}
                style={{ width: `${storagePercent}%` }}
              />
            </div>
            {storage?.quotaReached ? (
              <p className="mt-2 text-xs text-danger" role="alert">
                저장 한도에 도달했습니다. 완료·만료 전달을 정리한 뒤 새 전달을 준비하세요.
              </p>
            ) : null}
          </section>

          {message ? <p className="text-sm text-success" role="status">{message}</p> : null}
          {errorMessage ? <p className="text-sm text-danger" role="alert">{errorMessage}</p> : null}

          <section aria-labelledby="ai-handoff-history-list-title">
            <div className="mb-3 flex items-center gap-2">
              <Clock3 aria-hidden="true" />
              <h3 className="text-sm font-semibold" id="ai-handoff-history-list-title">최근 기록</h3>
            </div>
            {isLoading ? (
              <p className="rounded-md border border-border bg-white p-6 text-center text-sm text-muted">최근 전달을 불러오는 중입니다.</p>
            ) : items.length === 0 ? (
              <p className="rounded-md border border-border bg-white p-6 text-center text-sm text-muted">아직 전달 기록이 없습니다.</p>
            ) : (
              <ul className="grid gap-2" data-testid="ai-handoff-history-list">
                {items.map((item) => {
                  const usesGridInputActions = usesAiGridInputActions(item);
                  const available = item.payloadState === "available";
                  const working = workingRequestId === item.requestId;
                  return (
                    <li className="rounded-lg border border-border bg-white p-4" key={item.requestId}>
                      <div className="flex flex-wrap items-start justify-between gap-3">
                        <div className="min-w-0">
                          <p className="truncate text-sm font-semibold">
                            {item.iconName ?? "삭제된 아이콘"}
                            <span className="font-normal text-muted"> · {item.collectionName ?? "삭제된 모음"}</span>
                          </p>
                          <p className="mt-1 text-xs text-muted">
                            {handoffKindLabel(item)} · {serviceLabel(item.serviceSurface)} · {formatDateTime(item.createdAt)} · {payloadStateLabel(item)}
                          </p>
                        </div>
                        <div className="flex flex-wrap gap-2">
                          {available ? (
                            <>
                              <button
                                aria-describedby="ai-handoff-drag-keyboard-help"
                                className="inline-flex min-h-8 items-center gap-1 rounded-md bg-accent px-2.5 py-1.5 text-xs font-semibold text-accent-foreground hover:bg-accent-strong focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:opacity-50"
                                disabled={working || isMaintaining}
                                title="마우스로 누른 채 브라우저 업로드 영역까지 끌어 놓습니다."
                                type="button"
                                onClick={(event) => {
                                  if (event.detail === 0) void runItemAction(item, "reveal");
                                }}
                                onKeyDown={(event) => {
                                  if (event.key === "Enter" || event.key === " ") {
                                    event.preventDefault();
                                    void runItemAction(item, "reveal");
                                  }
                                }}
                                onPointerDown={(event) => {
                                  if (event.pointerType === "mouse" && event.button === 0) {
                                    void runItemAction(item, "drag");
                                  }
                                }}
                              >
                                <GripVertical aria-hidden="true" />파일 끌기
                              </button>
                              <button
                                className="inline-flex min-h-8 items-center gap-1 rounded-md border border-border px-2.5 py-1.5 text-xs font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:opacity-50"
                                disabled={working || isMaintaining}
                                type="button"
                                onClick={() => void runItemAction(item, "reveal")}
                              >
                                <FolderOpen aria-hidden="true" />탐색기
                              </button>
                              <button
                                aria-label={`${item.iconName ?? "전달"} ${usesGridInputActions ? "요청 취소" : "닫기"}`}
                                className="inline-flex min-h-8 items-center gap-1 rounded-md border border-border px-2.5 py-1.5 text-xs font-medium text-danger hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:opacity-50"
                                disabled={working || isMaintaining}
                                type="button"
                                onClick={() => void runItemAction(item, "delete")}
                              >
                                <Trash2 aria-hidden="true" />
                                {usesGridInputActions ? "요청 취소" : "닫기"}
                              </button>
                            </>
                          ) : null}
                          {item.collectionId ? (
                            <Link
                              className="inline-flex min-h-8 items-center rounded-md border border-border px-2.5 py-1.5 text-xs font-medium hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
                              params={{ collectionId: item.collectionId }}
                              to="/collections/$collectionId"
                              onClick={onClose}
                            >
                              모음 열기
                            </Link>
                          ) : null}
                        </div>
                      </div>
                    </li>
                  );
                })}
              </ul>
            )}
          </section>
        </div>
      </div>
    </div>
  );
}

function isAiGridHandoff(item: AiWebHandoffHistoryItem) {
  return (
    item.handoffKind === "ai_grid_sheet" ||
    ["grid_edit", "single_generate", "grid_generate"].includes(
      item.requestScope,
    )
  );
}

function usesAiGridInputActions(item: AiWebHandoffHistoryItem) {
  return isAiGridHandoff(item) && item.payloadState === "available";
}

function handoffKindLabel(item: AiWebHandoffHistoryItem) {
  return isAiGridHandoff(item) ? "AI 그리드" : "단일 아이콘";
}

function payloadStateLabel(item: AiWebHandoffHistoryItem) {
  if (item.payloadState === "cleanup_pending") {
    return item.hasResult ? "결과 받음 · 파일 정리 대기" : "파일 정리 대기";
  }
  if (item.payloadState === "deleted") {
    return item.hasResult ? "결과 받음 · 임시 파일 정리됨" : "임시 파일 정리됨";
  }
  if (item.hasResult) return "결과 받음";
  switch (item.payloadState) {
    case "available":
      return `결과 대기 · ${formatDateTime(item.expiresAt)} 만료`;
    case "expired":
      return "만료됨";
    default:
      return "닫힘";
  }
}

function serviceLabel(value: string) {
  if (value === "gemini_web") return "Gemini 웹";
  if (value === "novelai_web") return "NovelAI 웹";
  return "수동 웹";
}

function formatDateTime(value: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat("ko-KR", {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  }).format(date);
}

function formatBytes(bytes: number) {
  if (bytes >= 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(bytes >= 10 * 1024 * 1024 ? 0 : 1)}MB`;
  if (bytes >= 1024) return `${Math.round(bytes / 1024)}KB`;
  return `${bytes}B`;
}