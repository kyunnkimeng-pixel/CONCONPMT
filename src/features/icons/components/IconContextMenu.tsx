import { useEffect, useRef } from "react";
import type { ReactNode } from "react";
import { Copy, Pencil, Star, Trash2 } from "lucide-react";

import { cn } from "@/lib/utils";

interface IconContextMenuProps {
  x: number;
  y: number;
  isCover: boolean;
  selectionCount: number;
  onClose: () => void;
  onDelete: () => void;
  onDuplicate: () => void;
  onEdit: () => void;
  onSetCover: () => void;
}

export function IconContextMenu({
  x,
  y,
  isCover,
  selectionCount,
  onClose,
  onDelete,
  onDuplicate,
  onEdit,
  onSetCover,
}: IconContextMenuProps) {
  const menuRef = useRef<HTMLDivElement>(null);

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

  useEffect(() => {
    menuRef.current?.querySelector<HTMLButtonElement>("button:not(:disabled)")?.focus();
  }, []);

  const runAction = (action: () => void) => {
    action();
    onClose();
  };

  return (
    <div
      ref={menuRef}
      aria-label="아이콘 작업 메뉴"
      className="fixed min-w-48 rounded-md border border-border bg-white p-1 shadow-lg"
      role="menu"
      style={{ left: x, top: y }}
    >
      <MenuButton onClick={() => runAction(onEdit)}>
        <Pencil aria-hidden="true" />
        편집
      </MenuButton>
      <MenuButton onClick={() => runAction(onDuplicate)}>
        <Copy aria-hidden="true" />
        아이콘 복제
      </MenuButton>
      <MenuButton disabled={isCover} onClick={() => runAction(onSetCover)}>
        <Star aria-hidden="true" />
        {isCover ? "이미 대표 이미지" : "대표 이미지로 설정"}
      </MenuButton>
      <MenuButton tone="danger" onClick={() => runAction(onDelete)}>
        <Trash2 aria-hidden="true" />
        {selectionCount > 1 ? `선택 ${selectionCount}개 삭제` : "아이콘 삭제"}
      </MenuButton>
    </div>
  );
}

interface MenuButtonProps {
  children: ReactNode;
  disabled?: boolean;
  tone?: "default" | "danger";
  onClick: () => void;
}

function MenuButton({
  children,
  disabled = false,
  tone = "default",
  onClick,
}: MenuButtonProps) {
  return (
    <button
      className={cn(
        "flex w-full items-center gap-2 rounded px-3 py-2 text-left text-sm hover:bg-menu-hover focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus disabled:cursor-not-allowed disabled:text-muted",
        tone === "danger" ? "text-danger" : "text-foreground",
      )}
      disabled={disabled}
      role="menuitem"
      type="button"
      onClick={onClick}
    >
      {children}
    </button>
  );
}
