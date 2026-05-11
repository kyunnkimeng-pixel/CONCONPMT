import { useEffect, useRef, useState } from "react";

import { cn } from "@/lib/utils";

interface AltInlineEditorProps {
  value: string;
  ariaLabel: string;
  compact?: boolean;
  editRequestKey?: number;
  validationMessage: string | null;
  validateDraft: (value: string) => string | null;
  onCommit: (value: string) => Promise<boolean> | boolean;
}

export function AltInlineEditor({
  value,
  ariaLabel,
  compact = false,
  editRequestKey,
  validationMessage,
  validateDraft,
  onCommit,
}: AltInlineEditorProps) {
  const [isEditing, setIsEditing] = useState(false);
  const [draft, setDraft] = useState(value);
  const [draftError, setDraftError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const lastEditRequestKey = useRef(editRequestKey);
  const isCommitting = useRef(false);

  useEffect(() => {
    if (!isEditing) {
      setDraft(value);
    }
  }, [isEditing, value]);

  useEffect(() => {
    if (editRequestKey === undefined || editRequestKey === lastEditRequestKey.current) {
      return;
    }

    lastEditRequestKey.current = editRequestKey;
    setDraft(value);
    setDraftError(validateDraft(value));
    setIsEditing(true);
  }, [editRequestKey, validateDraft, value]);

  useEffect(() => {
    if (isEditing) {
      inputRef.current?.focus();
      inputRef.current?.select();
    }
  }, [isEditing]);

  const commit = async () => {
    if (isCommitting.current) {
      return;
    }

    setDraftError(validateDraft(draft));

    isCommitting.current = true;
    const didCommit = await onCommit(draft);
    isCommitting.current = false;

    if (didCommit) {
      setIsEditing(false);
      return;
    }

    setDraftError("alt 값을 저장할 수 없습니다.");
    inputRef.current?.focus();
  };

  const errorMessage = isEditing ? draftError : validationMessage;

  return (
    <div
      className={cn(
        "flex w-full min-w-0 flex-col items-center gap-1",
        compact ? "min-h-[30px]" : "min-h-[48px]",
      )}
    >
      {isEditing ? (
        <input
          ref={inputRef}
          aria-invalid={errorMessage ? true : undefined}
          aria-label={ariaLabel}
          className={cn(
            "w-full min-w-0 select-text rounded-md border bg-white px-2 text-center text-xs font-medium text-foreground outline-none",
            compact ? "py-1" : "py-1",
            errorMessage ? "border-danger" : "border-focus",
          )}
          data-testid="icon-alt-input"
          value={draft}
          onBlur={() => {
            void commit();
          }}
          onChange={(event) => {
            setDraft(event.target.value);
            setDraftError(validateDraft(event.target.value));
          }}
          onClick={(event) => event.stopPropagation()}
          onContextMenu={(event) => event.stopPropagation()}
          onDoubleClick={(event) => event.stopPropagation()}
          onDragStart={(event) => event.preventDefault()}
          onPointerDown={(event) => event.stopPropagation()}
          onKeyDown={(event) => {
            event.stopPropagation();

            if (event.key === "Enter") {
              event.preventDefault();
              void commit();
            }

            if (event.key === "Escape") {
              event.preventDefault();
              setDraft(value);
              setDraftError(null);
              setIsEditing(false);
            }
          }}
        />
      ) : (
        <button
          aria-label={ariaLabel}
          className={cn(
            "w-full min-w-0 truncate rounded-md px-2 py-1 text-center text-xs font-medium text-foreground hover:bg-white/80 focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus",
            errorMessage && "text-danger",
            !value && "text-muted",
          )}
          data-testid="icon-alt-button"
          type="button"
          onClick={(event) => {
            event.stopPropagation();
            setDraft(value);
            setDraftError(validateDraft(value));
            setIsEditing(true);
          }}
          onContextMenu={(event) => event.stopPropagation()}
          onDoubleClick={(event) => event.stopPropagation()}
          onDragStart={(event) => event.preventDefault()}
          onPointerDown={(event) => event.stopPropagation()}
          onKeyDown={(event) => event.stopPropagation()}
        >
          {value || "alt 입력"}
        </button>
      )}

      {errorMessage ? (
        <p className="line-clamp-2 text-center text-[11px] leading-tight text-danger" role="alert">
          {errorMessage}
        </p>
      ) : null}
    </div>
  );
}
