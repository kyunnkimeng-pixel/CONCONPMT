import type { SheetCell } from "@/features/sheets/types";
import { cn } from "@/lib/utils";

export function SheetCellTile({
  cell,
  included,
  onIncludedChange,
}: {
  cell: SheetCell;
  included: boolean;
  onIncludedChange: (included: boolean) => void;
}) {
  const status = cell.outOfBounds
    ? "invalid"
    : cell.emptyCandidate
      ? "empty"
      : included
        ? "selected"
        : "excluded";

  return (
    <label
      className={cn(
        "flex items-center gap-3 rounded-md border border-border bg-white p-3 text-sm",
        status === "empty" ? "opacity-60" : "",
        status === "invalid" ? "border-danger" : "",
      )}
    >
      <input
        checked={included}
        disabled={cell.outOfBounds}
        type="checkbox"
        onChange={(event) => onIncludedChange(event.currentTarget.checked)}
      />
      <span className="w-14 tabular-nums">#{cell.index + 1}</span>
      <span className="min-w-0 flex-1 truncate">
        x {cell.x}, y {cell.y}, {cell.w}×{cell.h}
      </span>
      <span className="rounded-md bg-preview px-2 py-1 text-xs text-muted">
        {status === "selected"
          ? "선택"
          : status === "empty"
            ? "빈 셀 후보"
            : status === "invalid"
              ? "범위 오류"
              : "제외"}
      </span>
    </label>
  );
}
