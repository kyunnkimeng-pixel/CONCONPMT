import { describe, expect, it } from "vitest";

import {
  isCoverImageFile,
  partitionImportableImageFiles,
  sortFilesForImport,
} from "@/lib/file-types";

function file(name: string, webkitRelativePath?: string) {
  const value = new File(["x"], name);
  Object.defineProperty(value, "webkitRelativePath", {
    configurable: true,
    value: webkitRelativePath ?? "",
  });
  return value;
}

describe("file-types", () => {
  it("filters importable image files for mixed folder imports", () => {
    const { accepted, rejected } = partitionImportableImageFiles([
      file("one.png"),
      file("two.gif"),
      file("notes.txt"),
    ]);

    expect(accepted.map((item) => item.name)).toEqual(["one.png", "two.gif"]);
    expect(rejected.map((item) => item.name)).toEqual(["notes.txt"]);
  });

  it("sorts folder imports by relative path for deterministic import order", () => {
    const sorted = sortFilesForImport([
      file("b.png", "icons/10-b.png"),
      file("a.png", "icons/02-a.png"),
      file("cover.png", "cover.png"),
    ]);

    expect(sorted.map((item) => item.name)).toEqual(["cover.png", "a.png", "b.png"]);
  });

  it("limits standalone cover files to jpg and png extensions", () => {
    expect(isCoverImageFile(file("cover.png"))).toBe(true);
    expect(isCoverImageFile(file("cover.jpg"))).toBe(true);
    expect(isCoverImageFile(file("cover.gif"))).toBe(false);
  });
});
