import { useState } from "react";
import { RotateCcw } from "lucide-react";

import { reimportEditSheet } from "@/features/sheets/api";
import { getCommandErrorMessage } from "@/lib/tauri";

export function SheetReimportDialog({
  collectionId,
  onImported,
}: {
  collectionId: string;
  onImported: () => Promise<void>;
}) {
  const [manifestFile, setManifestFile] = useState<File | null>(null);
  const [sheetFiles, setSheetFiles] = useState<File[]>([]);
  const [isRunning, setIsRunning] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const canRun = Boolean(manifestFile) && sheetFiles.length > 0 && !isRunning;

  return (
    <section className="flex flex-col gap-3 rounded-md border border-border bg-white p-4">
      <div>
        <h3 className="flex items-center gap-2 text-sm font-semibold">
          <RotateCcw aria-hidden="true" />
          수정된 시트 다시 가져오기
        </h3>
        <p className="mt-1 text-xs text-muted">
          pmtcon-sheet-v1 매니페스트와 수정한 clean sheet PNG를 선택합니다. 결과는 새 아이콘으로 등록되어 원본을 덮어쓰지 않습니다.
        </p>
      </div>
      <label className="flex flex-col gap-1 text-xs font-medium text-muted">
        Manifest JSON
        <input
          accept=".json,application/json"
          className="rounded-md border border-border bg-white px-2 py-2 text-sm text-foreground"
          type="file"
          onChange={(event) => setManifestFile(event.currentTarget.files?.[0] ?? null)}
        />
      </label>
      <label className="flex flex-col gap-1 text-xs font-medium text-muted">
        Edited clean sheet PNG
        <input
          accept=".png,image/png"
          className="rounded-md border border-border bg-white px-2 py-2 text-sm text-foreground"
          multiple
          type="file"
          onChange={(event) => setSheetFiles(Array.from(event.currentTarget.files ?? []))}
        />
      </label>
      <button
        className="rounded-md bg-accent px-3 py-2 text-sm font-semibold text-accent-foreground hover:bg-accent-strong disabled:cursor-not-allowed disabled:opacity-60"
        disabled={!canRun}
        type="button"
        onClick={() => {
          if (!manifestFile) {
            return;
          }
          setIsRunning(true);
          setMessage(null);
          setErrorMessage(null);
          void reimportEditSheet(collectionId, manifestFile, sheetFiles, "create_new_icons")
            .then(async (result) => {
              await onImported();
              setMessage(
                `${result.updatedItems.length}개 셀을 새 아이콘으로 가져왔습니다. ${result.skippedItems.length}개는 건너뛰었습니다.`,
              );
              if (result.errors.length > 0) {
                setErrorMessage(result.errors.join(" / "));
              }
            })
            .catch((error) => setErrorMessage(getCommandErrorMessage(error)))
            .finally(() => setIsRunning(false));
        }}
      >
        {isRunning ? "가져오는 중" : "매니페스트로 다시 가져오기"}
      </button>
      {message ? <p className="text-sm text-muted">{message}</p> : null}
      {errorMessage ? (
        <p className="text-sm text-danger" role="alert">
          {errorMessage}
        </p>
      ) : null}
    </section>
  );
}
