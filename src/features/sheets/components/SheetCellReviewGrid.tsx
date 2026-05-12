import type { SheetCell } from "@/features/sheets/types";
import { SheetCellTile } from "./SheetCellTile";

export function SheetCellReviewGrid({
  cells,
  selectedIndexes,
  onSelectionChange,
}: {
  cells: SheetCell[];
  selectedIndexes: Set<number>;
  onSelectionChange: (selectedIndexes: Set<number>) => void;
}) {
  const updateCell = (cellIndex: number, included: boolean) => {
    const next = new Set(selectedIndexes);
    if (included) {
      next.add(cellIndex);
    } else {
      next.delete(cellIndex);
    }
    onSelectionChange(next);
  };

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3 overflow-hidden">
      <div className="flex flex-wrap gap-2">
        <button
          className="rounded-md border border-border bg-white px-3 py-2 text-sm font-medium hover:bg-menu-hover"
          type="button"
          onClick={() => onSelectionChange(new Set(cells.filter((cell) => !cell.outOfBounds).map((cell) => cell.index)))}
        >
          전체 선택
        </button>
        <button
          className="rounded-md border border-border bg-white px-3 py-2 text-sm font-medium hover:bg-menu-hover"
          type="button"
          onClick={() => onSelectionChange(new Set())}
        >
          전체 해제
        </button>
        <button
          className="rounded-md border border-border bg-white px-3 py-2 text-sm font-medium hover:bg-menu-hover"
          type="button"
          onClick={() =>
            onSelectionChange(
              new Set(cells.filter((cell) => !selectedIndexes.has(cell.index) && !cell.outOfBounds).map((cell) => cell.index)),
            )
          }
        >
          선택 반전
        </button>
        <button
          className="rounded-md border border-border bg-white px-3 py-2 text-sm font-medium hover:bg-menu-hover"
          type="button"
          onClick={() =>
            onSelectionChange(new Set([...selectedIndexes].filter((index) => !cells.find((cell) => cell.index === index)?.emptyCandidate)))
          }
        >
          빈 셀 후보 제외
        </button>
      </div>
      <div className="min-h-0 flex-1 overflow-auto">
        <div className="grid gap-2 lg:grid-cols-2">
          {cells.map((cell) => (
            <SheetCellTile
              key={cell.index}
              cell={cell}
              included={selectedIndexes.has(cell.index)}
              onIncludedChange={(included) => updateCell(cell.index, included)}
            />
          ))}
        </div>
      </div>
    </div>
  );
}
