import type { ManualSliceDraft } from "@/features/sheets/manual-slice-model";

export function ManualSliceCanvas({ slices }: { slices: ManualSliceDraft[] }) {
  return (
    <div className="rounded-md border border-dashed border-border bg-preview p-4 text-sm text-muted">
      직접 Slice 지정은 future 범위입니다. 현재 저장된 draft slice 수: {slices.length}
    </div>
  );
}
