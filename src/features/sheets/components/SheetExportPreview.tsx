import { estimateSheetPages } from "@/features/sheets/sheet-ui-model";
import type { ExportEditSheetRequest } from "@/features/sheets/types";

export function SheetExportPreview({
  itemCount,
  request,
}: {
  itemCount: number;
  request: ExportEditSheetRequest;
}) {
  const pages = estimateSheetPages(itemCount, request);
  const maxColumns = Math.max(
    1,
    Math.floor(
      (request.maxSheetWidth - request.borderX * 2 + request.gapX) /
        (request.cellWidth + request.gapX),
    ),
  );
  const effectiveColumns = Math.min(request.columns, maxColumns);

  return (
    <section className="rounded-md border border-border bg-white p-4">
      <h3 className="text-sm font-semibold">시트 예상</h3>
      <dl className="mt-3 grid grid-cols-2 gap-3 text-sm">
        <div>
          <dt className="text-muted">대상</dt>
          <dd className="font-medium">{itemCount}개</dd>
        </div>
        <div>
          <dt className="text-muted">페이지</dt>
          <dd className="font-medium">{pages}개</dd>
        </div>
        <div>
          <dt className="text-muted">셀</dt>
          <dd className="font-medium">
            {request.cellWidth}×{request.cellHeight}
          </dd>
        </div>
        <div>
          <dt className="text-muted">열</dt>
          <dd className="font-medium">{effectiveColumns}</dd>
        </div>
      </dl>
      <p className="mt-3 text-xs text-muted">
        Clean sheet에는 라벨과 grid를 넣지 않습니다. Guide sheet에만 번호와 기준선을 표시합니다.
      </p>
    </section>
  );
}
