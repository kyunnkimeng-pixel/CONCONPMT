import { renderToString } from "react-dom/server";
import { describe, expect, it } from "vitest";

import {
  EditorOutputPreview,
  fitEditorOutputPreview,
} from "@/features/editor/components/EditorOutputPreview";

describe("EditorOutputPreview", () => {
  it("renders a sticky draft preview with multi-piece display and output sizes", () => {
    const html = renderToString(
      <EditorOutputPreview
        cellHeight={200}
        cellWidth={200}
        previewHeight={100}
        previewWidth={100}
        shape="horizontal_double"
      >
        <div>preview content</div>
      </EditorOutputPreview>,
    );
    const text = html.replace(/<!-- -->/g, "");

    expect(html).toContain('data-testid="editor-output-preview"');
    expect(html).toMatch(/class="[^"]*sticky[^"]*top-0[^"]*"/);
    expect(text).toContain("출력 미리보기");
    expect(text).toContain("적용 전");
    expect(text).toContain("표시 200×100px");
    expect(text).toContain("출력 조각 200×200px");
    expect(text).toContain("2개");
    expect(text).toContain("preview content");
  });

  it("scales a tall custom preview into the compact sticky area", () => {
    const html = renderToString(
      <EditorOutputPreview
        cellHeight={512}
        cellWidth={512}
        previewHeight={300}
        previewWidth={300}
        shape="vertical_double"
      >
        <div>tall preview</div>
      </EditorOutputPreview>,
    );
    const text = html.replace(/<!-- -->/g, "");

    expect(text).toContain("표시 300×600px");
    expect(html).toContain("overflow-hidden");

    const fittedPreview = fitEditorOutputPreview(300, 600);
    expect(fittedPreview.width).toBe(64);
    expect(fittedPreview.height).toBe(128);
    expect(fittedPreview.width).toBeLessThanOrEqual(220);
    expect(fittedPreview.height).toBeLessThanOrEqual(128);
  });
});
