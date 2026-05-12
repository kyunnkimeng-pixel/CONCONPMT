import { FileImage } from "lucide-react";

export function SheetImagePicker({
  file,
  onFileChange,
}: {
  file: File | null;
  onFileChange: (file: File | null) => void;
}) {
  return (
    <label className="flex cursor-pointer flex-col gap-3 rounded-md border border-dashed border-border bg-card p-4 hover:bg-menu-hover">
      <span className="flex items-center gap-2 text-sm font-semibold">
        <FileImage aria-hidden="true" />
        시트 이미지 선택
      </span>
      <span className="text-sm text-muted">
        PNG, JPG, JPEG 시트를 선택합니다. GIF는 GIF 프레임 시트 단계에서 별도로 처리합니다.
      </span>
      <input
        accept=".png,.jpg,.jpeg,image/png,image/jpeg"
        className="sr-only"
        type="file"
        onChange={(event) => {
          onFileChange(event.currentTarget.files?.[0] ?? null);
          event.currentTarget.value = "";
        }}
      />
      {file ? (
        <span className="truncate rounded-md border border-border bg-white px-3 py-2 text-sm">
          {file.name}
        </span>
      ) : null}
    </label>
  );
}
