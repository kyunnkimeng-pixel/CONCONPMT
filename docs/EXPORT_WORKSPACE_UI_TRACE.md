# Export Workspace UI Trace

Current stage: `EXPORT_WORKSPACE_UI_REFERENCE_AND_DESIGN`

Generated images are reference material only. They do not add product requirements and they do not remove required behavior from `docs/PRODUCT_SPEC.md`, `docs/FEATURE_INVENTORY.md`, `docs/ARCHITECTURE.md`, or the current design brief.

## Generated References

| Reference | Path | Notes |
| --- | --- | --- |
| A. Preflight screen | `docs/ui-references/export-workspace/01-export-workspace-preflight-before-after.png` | Revised to emphasize `전 / 후` image comparison and preflight status. |
| B. In-progress screen | `docs/ui-references/export-workspace/02-export-workspace-in-progress-before-after.png` | Revised to use one global progress bar and per-item statuses. |
| C. Completion/report screen | `docs/ui-references/export-workspace/03-export-workspace-completion-before-after.png` | Revised to keep output thumbnails visible beside the problem report. |
| D. GIF optimization panel | `docs/ui-references/export-workspace/04-gif-optimization-before-after.png` | Revised to compare original GIF and measured optimization candidate. |

## Requirement Trace

| Requirement | Source | Visible UI location | Planned component | Appears in generated reference | Generated-only ignored | Implementation status |
| --- | --- | --- | --- | --- | --- | --- |
| Clicking export opens a dedicated Export Workspace or export window. | Design brief | Whole workspace | `ExportWorkspaceRoute` or `ExportWorkspaceWindow` | yes | no | planned; current app has dialog-style export |
| Show actual emoticons that will be exported. | Design brief, PRODUCT_SPEC export contract | `전` pane and export result pane | `ExportGrid` | yes | no | planned; current export dialog has a simpler export piece grid |
| Let the user include/exclude individual export items. | Design brief, F074 | Export grid checkbox/card selection | `ExportGrid`, `update_export_item_included` | yes | no | partially existing via excluded piece ids; session model planned |
| Show item number. | Design brief | Export result pane | `ExportGrid` | yes | no | existing in export plan data; workspace presentation planned |
| Show thumbnail. | Design brief | Both before and after panes | `ExportGrid` | yes | no | existing preview data; larger workspace presentation planned |
| Show display name. | Design brief, PRODUCT_SPEC | Source item card and item inspector | `ExportGrid`, `ExportItemInspector` | yes | no | existing data; workspace presentation planned |
| Show alt value. | Design brief, PRODUCT_SPEC | Export result card/row | `ExportGrid` | yes | no | existing data; workspace presentation planned |
| Show format. | Design brief, F043 | Export result metadata | `ExportGrid` | yes | no | existing validation; workspace presentation planned |
| Show size and limit. | Design brief, F042 | Export result metadata and issue panel | `ExportGrid`, `ExportIssuePanel` | yes | no | existing validation; post-render workspace status planned |
| Show status. | Design brief | Badges on result cards/rows | `ExportStatusBadge` | yes | no | planned status model |
| Show actions for each item. | Design brief, F061 | Item inspector and action menu | `ExportItemInspector` | partial | no | planned; reveal/open actions must stay grounded in implemented commands |
| Distinguish export file generation from upload-ready validation. | Design policy | Status labels and reports | `ExportStatusBadge`, backend session model | yes | no | planned |
| Validation warnings do not block export by default. | Design brief, PRODUCT_SPEC, F056 | Preflight and report issue labels | `ExportIssuePanel` | yes | no | partially existing; workspace behavior planned |
| Upload-rule errors do not block whole export by default. | Design policy | `written_not_upload_ready` statuses | `ExportStatusBadge`, backend result model | yes | no | planned |
| If an item can be rendered/written, export it even if not upload-ready. | Design policy | Completion report | Backend job queue, `ExportCompletionReport` | yes | no | planned |
| Only render/write failures prevent that specific item from being written. | Design policy | Per-item failure status | Backend item result model | partial | no | planned |
| One failed item must not crash or stop the whole export job. | Design policy | Not primarily visual | Backend export session/job queue | no | no | planned; kept mandatory |
| Successful items receive check marks/status during export. | Design brief | In-progress result pane | `ExportStatusBadge` | yes | no | planned |
| File-size issues are marked separately. | Design brief, PRODUCT_SPEC, F042 | Result badges and problem list | `ExportIssuePanel` | yes | no | existing validation; workspace presentation planned |
| Validation errors are marked separately. | Design brief, PRODUCT_SPEC | Result badges and problem list | `ExportIssuePanel` | yes | no | existing validation; workspace presentation planned |
| Warnings are marked separately. | Design brief, PRODUCT_SPEC, F056 | Summary counts and badges | `ExportStatusBadge` | yes | no | existing validation; workspace presentation planned |
| User can select only problematic items and fix them. | Design brief | Problem filter and item inspector | `ExportCompletionReport`, `ExportItemInspector` | yes | no | planned |
| User can retry only problematic items. | Design brief | Completion report actions | `retry_export_items` | yes | no | planned |
| Bulk export avoids crashes. | Design brief | Not primarily visual | Backend queue and anti-crash strategy | no | no | planned; kept mandatory |
| GIFs over size limit are optimization candidates. | Design brief | GIF optimization panel | `GifOptimizationPanel` | yes | no | design only; MVP later |
| GIF optimization stores processed variants. | Design policy | GIF panel safety note | `processed_asset_variants` | yes | no | planned |
| Original source files are never overwritten. | PRODUCT_SPEC, product safety policy, design brief | GIF panel safety note and design docs | Backend variant model | yes | no | existing rule; workspace must preserve it |
| Export revalidates final written files. | Design policy | Report status after write | Backend validation pipeline | no | no | planned; kept mandatory |
| Sequence filename mode exists. | FEATURE_INVENTORY, PRODUCT_SPEC | Top toolbar | Filename mode selector | yes | no | existing |
| Alt-value filename mode exists. | FEATURE_INVENTORY, PRODUCT_SPEC | Top toolbar | Filename mode selector | yes | no | existing |
| `alts.txt` generation exists. | FEATURE_INVENTORY, PRODUCT_SPEC | Completion report actions | `ExportCompletionReport` | yes | no | existing |
| Export folder and alt txt open exist. | FEATURE_INVENTORY, PRODUCT_SPEC | Completion report actions | `reveal_export_result` | yes | no | existing; workspace command planned |
| Custom profile max bytes and strict warning settings remain supported. | FEATURE_INVENTORY | Toolbar/profile details or issue settings | Profile controls and validation options | partial | no | existing; detailed placement planned |
| GIF frame crop/resize and loop settings remain supported. | PRODUCT_SPEC, FEATURE_INVENTORY | Export backend and editor linkage | Export pipeline and item inspector | no | no | existing backend behavior; kept mandatory |
| F061 reveal original/per-icon export result remains in scope. | FEATURE_INVENTORY, design brief note | Item action menu/inspector | `reveal_export_item` | partial | no | keep in design; implementation status must follow inventory/code |
| F053 folder import is separate from Export Workspace. | Design brief | Not shown | none | no | no | intentionally separate issue |
| F062 standalone 200x200 JPG/PNG cover import is separate and is not the margin warning. | Design brief | Not shown | none | no | no | intentionally separate issue |
| No DCInside upload, login, posting, scraping, or browser automation. | product safety policy, design brief | No upload/login actions | none | yes, by omission | yes if generated | mandatory exclusion |
| No cloud/login/marketplace/premium/sync/account/community-upload features. | product safety policy, design brief | No such sidebar/menu/actions | none | yes, by omission | yes if generated | mandatory exclusion |
| No log-heavy primary UI. | User follow-up | Removed from main layout | `ExportProgressPanel` with one global bar | yes | no | design updated from first reference attempt |
| Use image visibility first. | User follow-up | Two-pane before/after layout | `ExportWorkspaceRoute`, `ExportGrid` | yes | no | design updated from first reference attempt |
| Avoid multiple progress/loading panes. | User follow-up | One global progress bar only | `ExportProgressPanel` | yes | no | design updated from first reference attempt |

## Ignored Generated Elements

- Exact sample thumbnail subject matter is decorative and not a product feature.
- Any misspelled or distorted generated Korean text is ignored; labels in implementation must follow the written docs.
- Exact colors, glass effects, shadows, rounded corners, and illustrative icons are style references only.
- Any generated cloud, login, account, marketplace, premium, upload, online sharing, social, remote storage, collaboration, scraping, browser automation, or AI-generation affordance is ignored and must not be implemented.
- Any action visible only in an image and not grounded in `PRODUCT_SPEC.md`, `FEATURE_INVENTORY.md`, `ARCHITECTURE.md`, or the design brief is ignored.

## Required Features Missing From Images But Kept

- Backend export session persistence and recovery.
- Item-level `Result` handling and no `unwrap`/`expect` in the export item pipeline.
- GIF concurrency limit of 1 and bounded PNG/JPG concurrency.
- Temp-file then atomic move behavior.
- Progress event throttling.
- Memory release after each item.
- Exact output directory structure: `files/`, `alts.txt`, `export_report.txt`, `export_report.json`, `export_issues.csv`.
- Full report field set: export index, icon id, display name, alt, filename, status, byte size, limit, warnings, errors, suggested fix.
- Final revalidation of written files.
- Database additions for sessions, session items, optional item results, processed variants, and optimization jobs.
- F053 folder import and F062 standalone cover import remain separate issues and are not removed by this design.

## Trace Conclusion

The revised visual direction should be used as a layout and readability reference only. The implementation stage should build the smallest MVP that supports a dedicated export workspace, before/after image visibility, include/exclude, partial-success export semantics, item-level results, one global progress indicator, reports, and GIF optimization design hooks without adding forbidden online or fake features.
