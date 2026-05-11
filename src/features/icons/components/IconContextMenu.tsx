import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { KeyboardEvent as ReactKeyboardEvent, ReactNode } from "react";
import {
  CheckCircle2,
  Copy,
  FileImage,
  FolderOpen,
  ImagePlus,
  Pencil,
  Star,
  Tags,
  Trash2,
} from "lucide-react";

import type { IconSummary } from "@/features/collections/types";

import { cn } from "@/lib/utils";

interface IconContextMenuProps {
  x: number;
  y: number;
  isCover: boolean;
  hasExportResult: boolean;
  selectionCount: number;
  altSelectionCount: number;
  onClose: () => void;
  onBatchAltEdit: () => void;
  onDelete: () => void;
  onDuplicate: () => void;
  onEdit: () => void;
  onRevealExportResult: () => void;
  onRevealOriginal: () => void;
  onReplaceImage: () => void;
  onRename: () => void;
  onSetCover: () => void;
  onSetReadiness: (readiness: IconSummary["readiness"]) => void;
  onSetThumbnailOverride: () => void;
}

export function IconContextMenu({
  x,
  y,
  isCover,
  hasExportResult,
  selectionCount,
  altSelectionCount,
  onClose,
  onBatchAltEdit,
  onDelete,
  onDuplicate,
  onEdit,
  onRevealExportResult,
  onRevealOriginal,
  onReplaceImage,
  onRename,
  onSetCover,
  onSetReadiness,
  onSetThumbnailOverride,
}: IconContextMenuProps) {
  const menuRef = useRef<HTMLDivElement>(null);
  const [position, setPosition] = useState({
    isMeasured: false,
    left: x,
    top: y,
  });

  useEffect(() => {
    const handlePointerDown = (event: PointerEvent) => {
      if (menuRef.current?.contains(event.target as Node)) {
        return;
      }

      onClose();
    };
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        onClose();
      }
    };

    window.addEventListener("pointerdown", handlePointerDown);
    window.addEventListener("keydown", handleKeyDown);

    return () => {
      window.removeEventListener("pointerdown", handlePointerDown);
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [onClose]);

  useLayoutEffect(() => {
    const menu = menuRef.current;
    if (!menu) {
      return;
    }

    const margin = 8;
    const rect = menu.getBoundingClientRect();
    const maxLeft = Math.max(margin, window.innerWidth - rect.width - margin);
    const maxTop = Math.max(margin, window.innerHeight - rect.height - margin);
    const left = Math.min(Math.max(x, margin), maxLeft);
    let top = y;

    if (rect.height + margin * 2 >= window.innerHeight) {
      top = margin;
    } else if (y > maxTop) {
      top = y - rect.height;
    }

    setPosition({
      isMeasured: true,
      left: Math.round(left),
      top: Math.round(Math.min(Math.max(top, margin), maxTop)),
    });
  }, [x, y]);

  useEffect(() => {
    if (!position.isMeasured) {
      return;
    }

    menuRef.current
      ?.querySelector<HTMLButtonElement>("button:not(:disabled)")
      ?.focus({ preventScroll: true });
  }, [position.isMeasured]);

  const focusMenuItem = (direction: "first" | "last" | "next" | "previous") => {
    const menuItems = Array.from(
      menuRef.current?.querySelectorAll<HTMLButtonElement>("button:not(:disabled)") ?? [],
    );

    if (menuItems.length === 0) {
      return;
    }

    const currentIndex = menuItems.indexOf(document.activeElement as HTMLButtonElement);
    const nextIndex =
      direction === "first"
        ? 0
        : direction === "last"
          ? menuItems.length - 1
          : direction === "next"
            ? (Math.max(currentIndex, 0) + 1) % menuItems.length
            : (currentIndex <= 0 ? menuItems.length : currentIndex) - 1;

    menuItems[nextIndex]?.focus({ preventScroll: true });
  };

  const handleMenuKeyDown = (event: ReactKeyboardEvent<HTMLDivElement>) => {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      focusMenuItem("next");
      return;
    }

    if (event.key === "ArrowUp") {
      event.preventDefault();
      focusMenuItem("previous");
      return;
    }

    if (event.key === "Home") {
      event.preventDefault();
      focusMenuItem("first");
      return;
    }

    if (event.key === "End") {
      event.preventDefault();
      focusMenuItem("last");
    }
  };

  const runAction = (action: () => void) => {
    action();
    onClose();
  };

  return (
    <div
      ref={menuRef}
      aria-label="아이콘 작업 메뉴"
      className="fixed z-50 max-h-[calc(100vh-16px)] min-w-48 overflow-y-auto rounded-md border border-border bg-white p-1 shadow-lg"
      data-testid="icon-context-menu"
      role="menu"
      style={{
        left: position.left,
        top: position.top,
        visibility: position.isMeasured ? "visible" : "hidden",
      }}
      onKeyDown={handleMenuKeyDown}
    >
      <MenuButton testId="icon-context-edit" onClick={() => runAction(onEdit)}>
        <Pencil aria-hidden="true" />
        편집
      </MenuButton>
      <MenuButton testId="icon-context-rename" onClick={() => runAction(onRename)}>
        <Pencil aria-hidden="true" />
        이름 변경
      </MenuButton>
      <MenuButton testId="icon-context-batch-alt" onClick={() => runAction(onBatchAltEdit)}>
        <Pencil aria-hidden="true" />
        {altSelectionCount > 1 ? `선택 ${altSelectionCount}개 alt 일괄 변경` : "alt 변경"}
      </MenuButton>
      <MenuButton testId="icon-context-thumbnail" onClick={() => runAction(onSetThumbnailOverride)}>
        <ImagePlus aria-hidden="true" />
        썸네일 바꾸기
      </MenuButton>
      <MenuButton testId="icon-context-replace-image" onClick={() => runAction(onReplaceImage)}>
        <FileImage aria-hidden="true" />
        이미지 대체하기
      </MenuButton>
      <MenuButton
        testId="icon-context-mark-working"
        onClick={() => runAction(() => onSetReadiness("working"))}
      >
        <Tags aria-hidden="true" />
        {selectionCount > 1 ? `선택 ${selectionCount}개 작업중` : "작업중으로 표시"}
      </MenuButton>
      <MenuButton
        testId="icon-context-mark-complete"
        onClick={() => runAction(() => onSetReadiness("complete"))}
      >
        <CheckCircle2 aria-hidden="true" />
        {selectionCount > 1 ? `선택 ${selectionCount}개 완성` : "완성으로 표시"}
      </MenuButton>
      <MenuButton testId="icon-context-duplicate" onClick={() => runAction(onDuplicate)}>
        <Copy aria-hidden="true" />
        아이콘 복제
      </MenuButton>
      <MenuButton disabled={isCover} testId="icon-context-set-cover" onClick={() => runAction(onSetCover)}>
        <Star aria-hidden="true" />
        {isCover ? "이미 대표 이미지" : "대표 이미지로 설정"}
      </MenuButton>
      <MenuButton testId="icon-context-reveal-original" onClick={() => runAction(onRevealOriginal)}>
        <FolderOpen aria-hidden="true" />
        원본 위치 열기
      </MenuButton>
      <MenuButton
        disabled={!hasExportResult}
        testId="icon-context-reveal-export"
        onClick={() => runAction(onRevealExportResult)}
      >
        <FileImage aria-hidden="true" />
        {hasExportResult ? "내보내기 결과 보기" : "내보내기 결과 없음"}
      </MenuButton>
      <MenuButton tone="danger" testId="icon-context-delete" onClick={() => runAction(onDelete)}>
        <Trash2 aria-hidden="true" />
        {selectionCount > 1 ? `선택 ${selectionCount}개 삭제` : "아이콘 삭제"}
      </MenuButton>
    </div>
  );
}

interface MenuButtonProps {
  children: ReactNode;
  disabled?: boolean;
  testId?: string;
  tone?: "default" | "danger";
  onClick: () => void;
}

function MenuButton({
  children,
  disabled = false,
  testId,
  tone = "default",
  onClick,
}: MenuButtonProps) {
  return (
    <button
      className={cn(
        "flex w-full items-center gap-2 rounded px-3 py-2 text-left text-sm hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:text-muted",
        tone === "danger" ? "text-danger" : "text-foreground",
      )}
      data-testid={testId}
      disabled={disabled}
      role="menuitem"
      type="button"
      onClick={onClick}
    >
      {children}
    </button>
  );
}
