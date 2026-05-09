import { Upload } from "lucide-react";
import type { DragEvent, ReactNode } from "react";

interface DropImportZoneProps {
  children: ReactNode;
  isDragging: boolean;
  label?: string;
  onDragStateChange: (isDragging: boolean) => void;
  onFilesDropped: (files: File[]) => void;
}

export function DropImportZone({
  children,
  isDragging,
  label = "이미지 파일 놓기",
  onDragStateChange,
  onFilesDropped,
}: DropImportZoneProps) {
  const handleDragOver = (event: DragEvent<HTMLDivElement>) => {
    event.preventDefault();
    onDragStateChange(true);
  };

  const handleDrop = (event: DragEvent<HTMLDivElement>) => {
    event.preventDefault();
    onDragStateChange(false);
    onFilesDropped(Array.from(event.dataTransfer.files));
  };

  return (
    <div
      className="relative min-h-[420px] rounded-xl border border-dashed border-dropzone bg-surface/75 p-5"
      onDragLeave={() => onDragStateChange(false)}
      onDragOver={handleDragOver}
      onDrop={handleDrop}
    >
      {children}

      {isDragging ? (
        <div className="absolute inset-3 flex items-center justify-center rounded-lg border border-focus bg-selected/90 text-focus">
          <div className="flex flex-col items-center gap-3 text-sm font-semibold">
            <Upload aria-hidden="true" />
            {label}
          </div>
        </div>
      ) : null}
    </div>
  );
}
