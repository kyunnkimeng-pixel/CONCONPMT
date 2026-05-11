import { Plus, Upload } from "lucide-react";
import { useRef } from "react";
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
  const dragDepthRef = useRef(0);

  const handleDragEnter = (event: DragEvent<HTMLDivElement>) => {
    if (!hasFileDrag(event.dataTransfer)) {
      return;
    }

    event.preventDefault();
    dragDepthRef.current += 1;
    onDragStateChange(true);
  };

  const handleDragOver = (event: DragEvent<HTMLDivElement>) => {
    if (!hasFileDrag(event.dataTransfer)) {
      return;
    }

    event.preventDefault();
    event.dataTransfer.dropEffect = "copy";
  };

  const handleDragLeave = (event: DragEvent<HTMLDivElement>) => {
    if (!hasFileDrag(event.dataTransfer)) {
      return;
    }

    event.preventDefault();
    dragDepthRef.current = Math.max(0, dragDepthRef.current - 1);
    if (dragDepthRef.current === 0) {
      onDragStateChange(false);
    }
  };

  const handleDrop = (event: DragEvent<HTMLDivElement>) => {
    event.preventDefault();
    dragDepthRef.current = 0;
    onDragStateChange(false);
    void droppedFiles(event.dataTransfer).then(onFilesDropped);
  };

  return (
    <div
      className="relative min-h-[420px] rounded-xl border border-dashed border-dropzone bg-surface/75 p-5"
      onDragEnter={handleDragEnter}
      onDragLeave={handleDragLeave}
      onDragOver={handleDragOver}
      onDrop={handleDrop}
    >
      {children}

      {isDragging ? (
        <div className="pointer-events-none absolute inset-3 flex items-center justify-center rounded-lg border-2 border-dashed border-focus bg-blue-100/80 text-focus shadow-inner">
          <div className="flex flex-col items-center gap-3 text-sm font-semibold">
            <span className="flex size-14 items-center justify-center rounded-full border border-focus bg-white/90 shadow-sm">
              <Plus aria-hidden="true" className="size-7" />
            </span>
            <span className="inline-flex items-center gap-2 rounded-full bg-white/90 px-4 py-2 shadow-sm">
              <Upload aria-hidden="true" className="size-4" />
              {label}
            </span>
          </div>
        </div>
      ) : null}
    </div>
  );
}

function hasFileDrag(dataTransfer: DataTransfer) {
  return Array.from(dataTransfer.types).includes("Files");
}

interface FileSystemEntryLike {
  isFile: boolean;
  isDirectory: boolean;
  name: string;
}

interface FileSystemFileEntryLike extends FileSystemEntryLike {
  file: (success: (file: File) => void, failure?: (error: DOMException) => void) => void;
}

interface FileSystemDirectoryEntryLike extends FileSystemEntryLike {
  createReader: () => {
    readEntries: (
      success: (entries: FileSystemEntryLike[]) => void,
      failure?: (error: DOMException) => void,
    ) => void;
  };
}

async function droppedFiles(dataTransfer: DataTransfer) {
  const entries: FileSystemEntryLike[] = [];
  for (const item of Array.from(dataTransfer.items)) {
    const getEntry = (
      item as DataTransferItem & {
        webkitGetAsEntry?: () => FileSystemEntryLike | null;
      }
    ).webkitGetAsEntry;
    const entry = typeof getEntry === "function" ? getEntry.call(item) : null;
    if (entry) {
      entries.push(entry);
    }
  }

  if (entries.length === 0) {
    return Array.from(dataTransfer.files);
  }

  const files = (
    await Promise.all(entries.map((entry) => filesFromEntry(entry, "")))
  ).flat();
  return files.length > 0 ? files : Array.from(dataTransfer.files);
}

async function filesFromEntry(entry: FileSystemEntryLike, parentPath: string): Promise<File[]> {
  const relativePath = parentPath ? `${parentPath}/${entry.name}` : entry.name;

  if (entry.isFile) {
    return [await fileFromEntry(entry as FileSystemFileEntryLike, relativePath)];
  }

  if (entry.isDirectory) {
    const children = await entriesFromDirectory(entry as FileSystemDirectoryEntryLike);
    const nested = await Promise.all(
      children.map((child) => filesFromEntry(child, relativePath)),
    );
    return nested.flat();
  }

  return [];
}

function fileFromEntry(entry: FileSystemFileEntryLike, relativePath: string) {
  return new Promise<File>((resolve, reject) => {
    entry.file(
      (file) => {
        Object.defineProperty(file, "webkitRelativePath", {
          configurable: true,
          value: relativePath,
        });
        resolve(file);
      },
      (error) => reject(error),
    );
  });
}

async function entriesFromDirectory(entry: FileSystemDirectoryEntryLike) {
  const reader = entry.createReader();
  const entries: FileSystemEntryLike[] = [];

  while (true) {
    const batch = await new Promise<FileSystemEntryLike[]>((resolve, reject) => {
      reader.readEntries(resolve, reject);
    });
    if (batch.length === 0) {
      break;
    }
    entries.push(...batch);
  }

  return entries;
}
