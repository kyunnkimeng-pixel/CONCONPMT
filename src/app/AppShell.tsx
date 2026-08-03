import { useEffect, useMemo, useState } from "react";
import { Link, Outlet, useRouterState } from "@tanstack/react-router";
import { CircleHelp, Clock3, ExternalLink, FolderOpen, Home, Images } from "lucide-react";

import { useAppStore } from "@/app/app-store";
import { AiHandoffHistoryDialog } from "@/features/ai-handoff-history/components/AiHandoffHistoryDialog";
import { listCollections } from "@/features/collections/api";
import { subscribeCollectionListChanged } from "@/features/collections/events";
import type { CollectionSummary } from "@/features/collections/types";

import { invokeCommand } from "@/lib/tauri";

export function AppShell() {
  const productName = useAppStore((state) => state.productName);
  const pathname = useRouterState({ select: (state) => state.location.pathname });
  const inCollection = pathname.startsWith("/collections/");
  const activeCollectionId = useMemo(() => {
    const match = pathname.match(/^\/collections\/([^/]+)/);
    return match ? decodeURIComponent(match[1]) : null;
  }, [pathname]);
  const [collections, setCollections] = useState<CollectionSummary[]>([]);
  const [helpError, setHelpError] = useState<string | null>(null);
  const [isAiHandoffHistoryOpen, setIsAiHandoffHistoryOpen] = useState(false);

  useEffect(() => {
    let isActive = true;
    let requestId = 0;

    const reloadCollections = () => {
      const currentRequestId = requestId + 1;
      requestId = currentRequestId;
      void listCollections()
        .then((nextCollections) => {
          if (isActive && requestId === currentRequestId) {
            setCollections(nextCollections);
          }
        })
        .catch(() => undefined);
    };

    reloadCollections();
    const unsubscribe = subscribeCollectionListChanged(reloadCollections);

    return () => {
      isActive = false;
      unsubscribe();
    };
  }, [pathname]);

  return (
    <main className="min-h-screen bg-background text-foreground">
      <div className="grid min-h-screen grid-cols-[240px_minmax(0,1fr)]">
        <aside className="sticky top-0 flex h-screen self-start flex-col overflow-y-auto border-r border-border/80 bg-sidebar px-3 py-4">
          <div className="mb-6 flex items-center gap-3 rounded-lg px-2">
            <div className="flex size-10 items-center justify-center rounded-md bg-accent text-accent-foreground shadow-sm">
              <Images aria-hidden="true" />
            </div>
            <div className="min-w-0">
              <p className="truncate text-sm font-semibold">{productName}</p>
              <p className="truncate text-xs text-muted">디시콘 제작 작업공간</p>
            </div>
          </div>

          <nav aria-label="주요 위치" className="flex flex-col gap-1">
            <Link
              aria-current={pathname === "/" ? "page" : undefined}
              className={
                pathname === "/"
                  ? "flex items-center gap-2 rounded-md bg-sidebar-active px-3 py-2 text-sm font-medium text-foreground"
                  : "flex items-center gap-2 rounded-md px-3 py-2 text-sm text-muted hover:bg-sidebar-active hover:text-foreground"
              }
              to="/"
            >
              <Home aria-hidden="true" />홈
            </Link>
            <div
              aria-current={inCollection ? "page" : undefined}
              className="flex items-center gap-2 rounded-md px-3 py-2 text-sm text-muted"
            >
              <FolderOpen aria-hidden="true" />
              모음
            </div>
            <div className="ml-4 flex max-h-[45vh] flex-col gap-1 overflow-auto pr-1">
              {collections.map((collection) => (
                <Link
                  aria-current={activeCollectionId === collection.id ? "page" : undefined}
                  className={
                    activeCollectionId === collection.id
                      ? "truncate rounded-md bg-sidebar-active px-3 py-1.5 text-sm font-medium text-foreground"
                      : "truncate rounded-md px-3 py-1.5 text-sm text-muted hover:bg-sidebar-active hover:text-foreground"
                  }
                  key={collection.id}
                  params={{ collectionId: collection.id }}
                  title={collection.name}
                  to="/collections/$collectionId"
                >
                  {collection.name}
                </Link>
              ))}
              {collections.length === 0 && !inCollection ? (
                <p className="px-3 py-1.5 text-xs text-muted">모음 없음</p>
              ) : null}
            </div>
          </nav>

          <nav aria-label="도움말" className="mt-auto border-t border-border/80 pt-3">
            <button
              className="flex w-full items-center gap-2 rounded-md px-3 py-2 text-left text-sm text-muted hover:bg-sidebar-active hover:text-foreground focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
              title="최근 웹 전달, 임시 저장 용량, 만료 정리 상태를 확인합니다."
              type="button"
              onClick={() => setIsAiHandoffHistoryOpen(true)}
            >
              <Clock3 aria-hidden="true" />
              최근 AI 전달
            </button>
            <button
              className="flex w-full items-center gap-2 rounded-md px-3 py-2 text-left text-sm text-muted hover:bg-sidebar-active hover:text-foreground focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
              title="시트 가져오기·내보내기, GIF 작업 시트, 메모 등 전체 사용법을 엽니다."
              type="button"
              onClick={() => {
                setHelpError(null);
                void invokeCommand("open_ai_official_resource", {
                  resource: "user_manual",
                }).catch(() => {
                  setHelpError("사용 설명서를 열지 못했습니다.");
                });
              }}
            >
              <CircleHelp aria-hidden="true" />
              사용 설명서
              <ExternalLink aria-hidden="true" className="ml-auto size-3.5" />
            </button>
            {helpError ? (
              <p className="px-3 pt-2 text-xs text-danger" role="alert">
                {helpError}
              </p>
            ) : null}
          </nav>
        </aside>

        <section className="min-w-0 bg-canvas">
          <Outlet />
        </section>
      </div>
      {isAiHandoffHistoryOpen ? (
        <AiHandoffHistoryDialog onClose={() => setIsAiHandoffHistoryOpen(false)} />
      ) : null}
    </main>
  );
}
