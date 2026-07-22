import { describe, expect, it } from "vitest";

import {
  formatImportResultMessage,
  importBatchSizeError,
  importFileSizeError,
  MAX_IMPORT_BATCH_BYTES,
  MAX_IMPORT_FILE_BYTES,
  partitionFilesByImportSize,
} from "@/lib/import-file";

describe("import file limits", () => {
  it("accepts a file at the limit", () => {
    expect(importFileSizeError({ name: "limit.png", size: MAX_IMPORT_FILE_BYTES })).toBeNull();
  });

  it("returns a clear Korean reason above the limit", () => {
    expect(
      importFileSizeError({ name: "large.png", size: MAX_IMPORT_FILE_BYTES + 1 }),
    ).toContain("최대 64MB");
  });

  it("separates oversized files before an import collection is created", () => {
    const valid = { name: "valid.png", size: 1 } as File;
    const oversized = {
      name: "large.png",
      size: MAX_IMPORT_FILE_BYTES + 1,
    } as File;

    expect(partitionFilesByImportSize([valid, oversized])).toEqual({
      accepted: [valid],
      rejected: [
        {
          originalFilename: "large.png",
          reason: "large.png: 원본 파일은 최대 64MB까지 가져올 수 있습니다.",
        },
      ],
    });
  });

  it("bounds multi-file payloads that must be sent together", () => {
    expect(
      importBatchSizeError([
        { size: MAX_IMPORT_BATCH_BYTES / 2 },
        { size: MAX_IMPORT_BATCH_BYTES / 2 + 1 },
      ]),
    ).toContain("파일의 합계");
  });

  it("summarizes only the first two skipped-file reasons", () => {
    expect(
      formatImportResultMessage(
        3,
        [{ name: "note.txt" }],
        [
          { originalFilename: "large.png", reason: "최대 64MB를 넘었습니다." },
          { originalFilename: "broken.gif", reason: "이미지를 해석할 수 없습니다." },
        ],
      ),
    ).toBe(
      "3개 이미지를 가져왔습니다. 3개 파일을 건너뛰었습니다: note.txt: 지원 형식이 아닙니다. / large.png: 최대 64MB를 넘었습니다. 외 1개",
    );
  });
});
