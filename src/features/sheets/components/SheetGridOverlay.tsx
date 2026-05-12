import type { SheetCell } from "@/features/sheets/types";
import { cn } from "@/lib/utils";

export function SheetGridOverlay({
  imageUrl,
  sheetWidth,
  sheetHeight,
  cells,
  selectedIndexes,
  onToggleCell,
}: {
  imageUrl: string | null;
  sheetWidth: number;
  sheetHeight: number;
  cells: SheetCell[];
  selectedIndexes: Set<number>;
  onToggleCell: (cellIndex: number, multi: boolean) => void;
}) {
  return (
    <div className="min-h-[420px] overflow-auto bg-preview p-4">
      <div
        className="relative mx-auto max-w-full"
        style={{
          aspectRatio: sheetWidth > 0 && sheetHeight > 0 ? `${sheetWidth}/${sheetHeight}` : "1",
          width: sheetWidth > 0 ? Math.min(sheetWidth, 960) : 640,
        }}
      >
        {imageUrl ? (
          <img
            alt="스프라이트 시트"
            className="absolute inset-0 size-full object-contain"
            draggable={false}
            src={imageUrl}
          />
        ) : (
          <div className="absolute inset-0 flex items-center justify-center text-sm text-muted">
            시트 이미지를 선택하세요.
          </div>
        )}
        {sheetWidth > 0 && sheetHeight > 0 ? (
          <svg
            className="absolute inset-0 size-full"
            preserveAspectRatio="none"
            viewBox={`0 0 ${sheetWidth} ${sheetHeight}`}
          >
            {cells.map((cell) => {
              const selected = selectedIndexes.has(cell.index);
              return (
                <g key={cell.index}>
                  <rect
                    className={cn(
                      "cursor-pointer",
                      cell.outOfBounds ? "fill-danger/20" : "fill-transparent",
                    )}
                    height={cell.h}
                    stroke={
                      cell.outOfBounds ? "rgb(185 28 28)" : selected ? "rgb(30 98 214)" : "rgb(31 41 55)"
                    }
                    strokeDasharray={cell.emptyCandidate ? "6 4" : undefined}
                    strokeWidth={selected ? 3 : 1}
                    width={cell.w}
                    x={cell.x}
                    y={cell.y}
                    onClick={(event) => onToggleCell(cell.index, event.ctrlKey || event.metaKey)}
                  />
                  {selected ? (
                    <rect
                      fill="rgba(30, 98, 214, 0.18)"
                      height={cell.h}
                      pointerEvents="none"
                      width={cell.w}
                      x={cell.x}
                      y={cell.y}
                    />
                  ) : null}
                  {cell.emptyCandidate ? (
                    <rect
                      fill="rgba(255,255,255,0.45)"
                      height={cell.h}
                      pointerEvents="none"
                      width={cell.w}
                      x={cell.x}
                      y={cell.y}
                    />
                  ) : null}
                  <text
                    fill="rgb(17 24 39)"
                    fontSize={Math.max(10, Math.min(cell.w, cell.h) * 0.12)}
                    pointerEvents="none"
                    x={cell.x + 4}
                    y={cell.y + 14}
                  >
                    {cell.index + 1}
                  </text>
                </g>
              );
            })}
          </svg>
        ) : null}
      </div>
    </div>
  );
}
