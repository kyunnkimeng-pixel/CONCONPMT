import { useEffect, useRef, useState } from "react";

interface InlineNameEditorProps {
  value: string;
  ariaLabel: string;
  onCommit: (value: string) => void;
}

export function InlineNameEditor({ value, ariaLabel, onCommit }: InlineNameEditorProps) {
  const [isEditing, setIsEditing] = useState(false);
  const [draft, setDraft] = useState(value);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    setDraft(value);
  }, [value]);

  useEffect(() => {
    if (isEditing) {
      inputRef.current?.focus();
      inputRef.current?.select();
    }
  }, [isEditing]);

  const commit = () => {
    setIsEditing(false);
    onCommit(draft);
  };

  if (isEditing) {
    return (
      <input
        ref={inputRef}
        aria-label={ariaLabel}
        className="w-full rounded-md border border-focus bg-white px-2 py-1 text-center text-sm font-semibold text-foreground outline-none"
        value={draft}
        onChange={(event) => setDraft(event.target.value)}
        onBlur={commit}
        onClick={(event) => event.stopPropagation()}
        onDoubleClick={(event) => event.stopPropagation()}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            commit();
          }

          if (event.key === "Escape") {
            setDraft(value);
            setIsEditing(false);
          }
        }}
      />
    );
  }

  return (
    <button
      aria-label={ariaLabel}
      className="w-full truncate rounded-md px-2 py-1 text-center text-sm font-semibold text-foreground hover:bg-white/80 focus-visible:outline focus-visible:outline-2 focus-visible:outline-focus"
      type="button"
      onClick={(event) => {
        event.stopPropagation();
        setIsEditing(true);
      }}
      onDoubleClick={(event) => event.stopPropagation()}
    >
      {value}
    </button>
  );
}
