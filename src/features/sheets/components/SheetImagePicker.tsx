import { useState } from "react";
import type { DragEvent } from "react";
import { FileImage } from "lucide-react";

import { cn } from "@/lib/utils";

export function SheetImagePicker({
  file,
  onFileChange,
}: {
  file: File | null;
  onFileChange: (file: File | null) => void;
}) {
  const [isDragging, setIsDragging] = useState(false);
  const isJpegSheet = file ? /\.(jpe?g)$/i.test(file.name) || file.type === "image/jpeg" : false;

  const handleDrop = (event: DragEvent<HTMLLabelElement>) => {
    event.preventDefault();
    setIsDragging(false);
    const droppedFile = Array.from(event.dataTransfer.files).find(isSheetImageFile);
    if (droppedFile) {
      onFileChange(droppedFile);
    }
  };

  return (
    <label
      className={cn(
        "flex cursor-pointer flex-col gap-3 rounded-md border border-dashed border-border bg-card p-4 hover:bg-menu-hover",
        isDragging ? "border-focus bg-selected" : "",
      )}
      onDragEnter={(event) => {
        event.preventDefault();
        setIsDragging(true);
      }}
      onDragLeave={(event) => {
        if (event.currentTarget === event.target) {
          setIsDragging(false);
        }
      }}
      onDragOver={(event) => {
        event.preventDefault();
        event.dataTransfer.dropEffect = "copy";
      }}
      onDrop={handleDrop}
    >
      <span className="flex items-center gap-2 text-sm font-semibold">
        <FileImage aria-hidden="true" />
        시트 이미지 선택
      </span>
      <span className="text-sm text-muted">
        PNG, JPG, JPEG 시트를 선택하거나 여기로 드래그해서 놓습니다. GIF는 GIF 프레임 시트 단계에서 별도로 처리합니다.
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
      {isJpegSheet ? (
        <span className="rounded-md border border-amber-200 bg-amber-50 px-3 py-2 text-xs text-amber-800">
          JPG/JPEG 시트는 alpha 투명도를 포함하지 않습니다. 투명 배경 편집이 필요하면 PNG 시트를 사용하세요.
        </span>
      ) : null}
    </label>
  );
}

function isSheetImageFile(file: File) {
  return /\.(png|jpe?g)$/i.test(file.name) || file.type === "image/png" || file.type === "image/jpeg";
}
