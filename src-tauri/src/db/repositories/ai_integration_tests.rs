use std::fs;
use std::io::Cursor;
use std::time::{SystemTime, UNIX_EPOCH};

use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};
use sha2::{Digest, Sha256};

use crate::db::connection::open_database;
use crate::db::repositories::ai::{
    activate_ai_candidate, create_ai_icon_root, get_ai_review_state, import_local_ai_candidate,
    preview_ai_candidate_normalization, repair_ai_to_original, restore_ai_version,
    MAX_AI_CANDIDATE_BYTES,
};
use crate::db::repositories::collections::{create_collection, duplicate_collection};
use crate::db::repositories::editor::get_icon_editor_state;
use crate::db::repositories::icons::{delete_icons, duplicate_icon, replace_icon_source};
use crate::db::repositories::imports::import_image_files;
use crate::models::{
    ActivateAiCandidatePayload, AiNormalizationOptionsPayload, AiSourceMutationResultDto,
    CreateAiIconRootPayload, ImportAiCandidatePayload, ImportImageFilePayload,
    PreviewAiCandidateNormalizationPayload, RepairAiToOriginalPayload, RestoreAiVersionPayload,
};
use crate::paths::AppPaths;

fn png_bytes(color: [u8; 4]) -> Vec<u8> {
    png_bytes_with_dimensions(8, 8, color)
}

fn png_bytes_with_dimensions(width: u32, height: u32, color: [u8; 4]) -> Vec<u8> {
    let image = ImageBuffer::from_pixel(width, height, Rgba(color));
    let mut cursor = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image)
        .write_to(&mut cursor, ImageFormat::Png)
        .unwrap();
    cursor.into_inner()
}

fn gif_bytes() -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut encoder = gif::Encoder::new(&mut bytes, 8, 8, &[]).unwrap();
        encoder.set_repeat(gif::Repeat::Infinite).unwrap();
        let mut pixels = vec![255_u8; 8 * 8 * 4];
        let mut frame = gif::Frame::from_rgba_speed(8, 8, &mut pixels, 10);
        frame.delay = 5;
        encoder.write_frame(&frame).unwrap();
    }
    bytes
}

fn animated_gif_bytes() -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut encoder = gif::Encoder::new(&mut bytes, 8, 8, &[]).unwrap();
        encoder.set_repeat(gif::Repeat::Infinite).unwrap();
        let mut first_pixels = vec![255_u8; 8 * 8 * 4];
        let mut first = gif::Frame::from_rgba_speed(8, 8, &mut first_pixels, 10);
        first.delay = 5;
        encoder.write_frame(&first).unwrap();
        let mut second_pixels = Vec::with_capacity(8 * 8 * 4);
        for _ in 0..(8 * 8) {
            second_pixels.extend_from_slice(&[20, 40, 220, 255]);
        }
        let mut second = gif::Frame::from_rgba_speed(8, 8, &mut second_pixels, 10);
        second.delay = 7;
        encoder.write_frame(&second).unwrap();
    }
    bytes
}

fn temp_paths() -> AppPaths {
    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    AppPaths::prepare(std::env::temp_dir().join(format!(
        "pmtconcon-ai-restart-{}-{suffix}",
        std::process::id()
    )))
    .unwrap()
}

fn assert_ai_mutation_snapshots_match(result: &AiSourceMutationResultDto, expected_icon_id: &str) {
    let review_source = &result.review_state.visual_source;
    let editor_source = &result.editor_state.visual_source;
    assert_eq!(result.editor_state.icon.id, expected_icon_id);
    assert_eq!(
        result.editor_state.source.id,
        review_source.effective_render_source.id
    );
    assert_eq!(
        editor_source.original_source.id,
        review_source.original_source.id
    );
    assert_eq!(
        editor_source.effective_render_source.id,
        review_source.effective_render_source.id
    );
    assert_eq!(
        editor_source.effective_render_source.sha256,
        review_source.effective_render_source.sha256
    );
    assert_eq!(
        editor_source.active_version_id,
        review_source.active_version_id
    );
    assert_eq!(
        editor_source.active_candidate_id,
        review_source.active_candidate_id
    );
    assert_eq!(
        editor_source.activation_revision,
        review_source.activation_revision
    );
    assert_eq!(
        editor_source.normalization_recipe_hash,
        review_source.normalization_recipe_hash
    );
}
#[test]
fn ai_activation_and_original_or_previous_version_restore_survive_restart() {
    let paths = temp_paths();
    let (collection_id, icon_id, original_source_id, version_id, ai_source_id) = {
        let mut connection = open_database(&paths.database_path).unwrap();
        let collection =
            create_collection(&mut connection, Some("AI 재시작 복원".to_string())).unwrap();
        let icon = import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![ImportImageFilePayload {
                original_filename: "original.png".to_string(),
                bytes: png_bytes([20, 40, 220, 255]),
            }],
        )
        .unwrap()
        .imported_icons
        .into_iter()
        .next()
        .unwrap();
        let original_source_id = icon.source_file_id.clone();
        let review = import_local_ai_candidate(
            &mut connection,
            &paths,
            &collection.id,
            ImportAiCandidatePayload {
                icon_id: icon.id.clone(),
                service_surface: "gemini_web".to_string(),
                file: ImportImageFilePayload {
                    original_filename: "candidate.png".to_string(),
                    bytes: png_bytes([230, 80, 30, 180]),
                },
            },
        )
        .unwrap();
        let candidate_id = review.candidates[0].id.clone();
        let activated = activate_ai_candidate(
            &mut connection,
            &paths,
            &collection.id,
            ActivateAiCandidatePayload {
                icon_id: icon.id.clone(),
                candidate_id: candidate_id.clone(),
                expected_revision: review.visual_source.activation_revision,
                normalization: Default::default(),
                expected_preview_signature: None,
            },
        )
        .unwrap();
        assert_ai_mutation_snapshots_match(&activated, &icon.id);
        let version = activated
            .review_state
            .versions
            .iter()
            .find(|version| version.candidate_id == candidate_id)
            .unwrap();
        assert!(version.is_active);
        assert_ne!(
            activated
                .review_state
                .visual_source
                .effective_render_source
                .id,
            original_source_id
        );
        (
            collection.id,
            icon.id,
            original_source_id,
            version.id.clone(),
            activated
                .review_state
                .visual_source
                .effective_render_source
                .id
                .clone(),
        )
    };

    {
        let mut connection = open_database(&paths.database_path).unwrap();
        let persisted = get_ai_review_state(&connection, &collection_id, &icon_id).unwrap();
        assert_eq!(
            persisted.visual_source.active_version_id.as_deref(),
            Some(version_id.as_str())
        );
        assert_eq!(
            persisted.visual_source.effective_render_source.id,
            ai_source_id
        );
        let restored = restore_ai_version(
            &mut connection,
            &paths,
            &collection_id,
            RestoreAiVersionPayload {
                icon_id: icon_id.clone(),
                version_id: None,
                expected_revision: persisted.visual_source.activation_revision,
            },
        )
        .unwrap();
        assert_ai_mutation_snapshots_match(&restored, &icon_id);
        assert_eq!(restored.review_state.visual_source.active_version_id, None);
        assert_eq!(
            restored
                .review_state
                .visual_source
                .effective_render_source
                .id,
            original_source_id
        );
    }

    {
        let mut connection = open_database(&paths.database_path).unwrap();
        let original = get_ai_review_state(&connection, &collection_id, &icon_id).unwrap();
        let restored = restore_ai_version(
            &mut connection,
            &paths,
            &collection_id,
            RestoreAiVersionPayload {
                icon_id: icon_id.clone(),
                version_id: Some(version_id.clone()),
                expected_revision: original.visual_source.activation_revision,
            },
        )
        .unwrap();
        assert_ai_mutation_snapshots_match(&restored, &icon_id);
        assert_eq!(
            restored
                .review_state
                .visual_source
                .active_version_id
                .as_deref(),
            Some(version_id.as_str())
        );
        assert_eq!(
            restored
                .review_state
                .visual_source
                .effective_render_source
                .id,
            ai_source_id
        );
        assert_eq!(restored.review_state.versions.len(), 1);
    }

    fs::remove_dir_all(paths.root).unwrap();
}

#[test]
fn applying_one_candidate_marks_sibling_candidates_stale_even_after_original_restore() {
    let paths = temp_paths();
    {
        let mut connection = open_database(&paths.database_path).unwrap();
        let collection =
            create_collection(&mut connection, Some("AI 후보 비교".to_string())).unwrap();
        let icon = import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![ImportImageFilePayload {
                original_filename: "original.png".to_string(),
                bytes: png_bytes([20, 40, 220, 255]),
            }],
        )
        .unwrap()
        .imported_icons
        .into_iter()
        .next()
        .unwrap();

        let first_review = import_local_ai_candidate(
            &mut connection,
            &paths,
            &collection.id,
            ImportAiCandidatePayload {
                icon_id: icon.id.clone(),
                service_surface: "gemini_web".to_string(),
                file: ImportImageFilePayload {
                    original_filename: "candidate-a.png".to_string(),
                    bytes: png_bytes([230, 80, 30, 180]),
                },
            },
        )
        .unwrap();
        let first_candidate_id = first_review
            .candidates
            .iter()
            .find(|candidate| candidate.source.original_filename == "candidate-a.png")
            .unwrap()
            .id
            .clone();

        let second_review = import_local_ai_candidate(
            &mut connection,
            &paths,
            &collection.id,
            ImportAiCandidatePayload {
                icon_id: icon.id.clone(),
                service_surface: "novelai_web".to_string(),
                file: ImportImageFilePayload {
                    original_filename: "candidate-b.png".to_string(),
                    bytes: png_bytes([30, 210, 100, 255]),
                },
            },
        )
        .unwrap();
        let second_candidate_id = second_review
            .candidates
            .iter()
            .find(|candidate| candidate.source.original_filename == "candidate-b.png")
            .unwrap()
            .id
            .clone();
        assert!(second_review
            .candidates
            .iter()
            .all(|candidate| !candidate.is_stale));

        let activated = activate_ai_candidate(
            &mut connection,
            &paths,
            &collection.id,
            ActivateAiCandidatePayload {
                icon_id: icon.id.clone(),
                candidate_id: first_candidate_id,
                expected_revision: second_review.visual_source.activation_revision,
                normalization: Default::default(),
                expected_preview_signature: None,
            },
        )
        .unwrap();
        let stale_sibling = activated
            .review_state
            .candidates
            .iter()
            .find(|candidate| candidate.id == second_candidate_id)
            .unwrap();
        assert!(stale_sibling.is_stale);
        assert!(stale_sibling.stale_reason.is_some());

        let stale_error = activate_ai_candidate(
            &mut connection,
            &paths,
            &collection.id,
            ActivateAiCandidatePayload {
                icon_id: icon.id.clone(),
                candidate_id: second_candidate_id.clone(),
                expected_revision: activated.review_state.visual_source.activation_revision,
                normalization: Default::default(),
                expected_preview_signature: None,
            },
        )
        .unwrap_err();
        assert_eq!(stale_error.code, "ai_candidate_stale");

        let restored = restore_ai_version(
            &mut connection,
            &paths,
            &collection.id,
            RestoreAiVersionPayload {
                icon_id: icon.id,
                version_id: None,
                expected_revision: activated.review_state.visual_source.activation_revision,
            },
        )
        .unwrap();
        let stale_after_restore = restored
            .review_state
            .candidates
            .iter()
            .find(|candidate| candidate.id == second_candidate_id)
            .unwrap();
        assert!(stale_after_restore.is_stale);
    }
    fs::remove_dir_all(paths.root).unwrap();
}

#[test]
fn ai_candidate_import_accepts_arbitrary_static_canvas_but_rejects_gif_without_mutation() {
    let paths = temp_paths();
    {
        let mut connection = open_database(&paths.database_path).unwrap();
        let collection =
            create_collection(&mut connection, Some("AI 후보 입력 제한".to_string())).unwrap();
        let icon = import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![ImportImageFilePayload {
                original_filename: "original.png".to_string(),
                bytes: png_bytes([20, 40, 220, 255]),
            }],
        )
        .unwrap()
        .imported_icons
        .into_iter()
        .next()
        .unwrap();

        let arbitrary_size = import_local_ai_candidate(
            &mut connection,
            &paths,
            &collection.id,
            ImportAiCandidatePayload {
                icon_id: icon.id.clone(),
                service_surface: "gemini_web".to_string(),
                file: ImportImageFilePayload {
                    original_filename: "wrong-size.png".to_string(),
                    bytes: png_bytes_with_dimensions(9, 8, [230, 80, 30, 255]),
                },
            },
        )
        .unwrap();
        let arbitrary_candidate = arbitrary_size
            .candidates
            .iter()
            .find(|candidate| candidate.source.original_filename == "wrong-size.png")
            .unwrap();
        assert_eq!(
            (
                arbitrary_candidate.source.width,
                arbitrary_candidate.source.height
            ),
            (9, 8)
        );
        let counts_before_gif: (i64, i64, i64) = connection
            .query_row(
                "SELECT
                   (SELECT COUNT(*) FROM ai_requests),
                   (SELECT COUNT(*) FROM ai_candidates),
                   (SELECT COUNT(*) FROM source_files)",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();

        let animated = import_local_ai_candidate(
            &mut connection,
            &paths,
            &collection.id,
            ImportAiCandidatePayload {
                icon_id: icon.id,
                service_surface: "novelai_web".to_string(),
                file: ImportImageFilePayload {
                    original_filename: "animated.gif".to_string(),
                    bytes: gif_bytes(),
                },
            },
        )
        .unwrap_err();
        assert_eq!(animated.code, "validation");

        let counts_after_gif: (i64, i64, i64) = connection
            .query_row(
                "SELECT
                   (SELECT COUNT(*) FROM ai_requests),
                   (SELECT COUNT(*) FROM ai_candidates),
                   (SELECT COUNT(*) FROM source_files)",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(counts_before_gif, (1, 1, 2));
        assert_eq!(counts_after_gif, counts_before_gif);
    }
    fs::remove_dir_all(paths.root).unwrap();
}

#[test]
fn missing_active_ai_file_can_be_repaired_to_original_after_restart() {
    let paths = temp_paths();
    let (collection_id, icon_id, original_source_id, active_source_path) = {
        let mut connection = open_database(&paths.database_path).unwrap();
        let collection =
            create_collection(&mut connection, Some("AI 손상 복구".to_string())).unwrap();
        let icon = import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![ImportImageFilePayload {
                original_filename: "original.png".to_string(),
                bytes: png_bytes([20, 40, 220, 255]),
            }],
        )
        .unwrap()
        .imported_icons
        .into_iter()
        .next()
        .unwrap();
        let review = import_local_ai_candidate(
            &mut connection,
            &paths,
            &collection.id,
            ImportAiCandidatePayload {
                icon_id: icon.id.clone(),
                service_surface: "gemini_web".to_string(),
                file: ImportImageFilePayload {
                    original_filename: "candidate.png".to_string(),
                    bytes: png_bytes([230, 80, 30, 180]),
                },
            },
        )
        .unwrap();
        let activated = activate_ai_candidate(
            &mut connection,
            &paths,
            &collection.id,
            ActivateAiCandidatePayload {
                icon_id: icon.id.clone(),
                candidate_id: review.candidates[0].id.clone(),
                expected_revision: review.visual_source.activation_revision,
                normalization: Default::default(),
                expected_preview_signature: None,
            },
        )
        .unwrap();
        let active_source_path: String = connection
            .query_row(
                "SELECT original_path_in_library FROM source_files WHERE id = ?1",
                [activated
                    .review_state
                    .visual_source
                    .effective_render_source
                    .id
                    .as_str()],
                |row| row.get(0),
            )
            .unwrap();
        (
            collection.id,
            icon.id,
            activated.review_state.visual_source.original_source.id,
            active_source_path,
        )
    };

    fs::remove_file(&active_source_path).unwrap();
    {
        let mut connection = open_database(&paths.database_path).unwrap();
        let review_error = get_ai_review_state(&connection, &collection_id, &icon_id).unwrap_err();
        assert_eq!(review_error.code, "ai_source_repair_required");
        let editor_error =
            get_icon_editor_state(&connection, &collection_id, &icon_id).unwrap_err();
        assert_eq!(editor_error.code, "ai_source_repair_required");

        let repaired = repair_ai_to_original(
            &mut connection,
            &paths,
            &collection_id,
            RepairAiToOriginalPayload {
                icon_id: icon_id.clone(),
            },
        )
        .unwrap();
        assert_eq!(repaired.visual_source.active_version_id, None);
        assert_eq!(
            repaired.visual_source.effective_render_source.id,
            original_source_id
        );
        get_icon_editor_state(&connection, &collection_id, &icon_id).unwrap();
    }
    fs::remove_dir_all(paths.root).unwrap();
}

#[test]
fn oversized_ai_candidate_is_rejected_before_decode_or_database_mutation() {
    let paths = temp_paths();
    {
        let mut connection = open_database(&paths.database_path).unwrap();
        let error = import_local_ai_candidate(
            &mut connection,
            &paths,
            "missing_collection",
            ImportAiCandidatePayload {
                icon_id: "missing_icon".to_string(),
                service_surface: "gemini_web".to_string(),
                file: ImportImageFilePayload {
                    original_filename: "too-large.png".to_string(),
                    bytes: vec![0; MAX_AI_CANDIDATE_BYTES + 1],
                },
            },
        )
        .unwrap_err();
        assert_eq!(error.code, "ai_candidate_too_large");
        let request_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM ai_requests", [], |row| row.get(0))
            .unwrap();
        assert_eq!(request_count, 0);
    }
    fs::remove_dir_all(paths.root).unwrap();
}
#[test]
fn old_lineage_candidate_creates_independent_working_icon_with_cloned_dag() {
    let paths = temp_paths();
    {
        let mut connection = open_database(&paths.database_path).unwrap();
        let collection =
            create_collection(&mut connection, Some("AI 새 아이콘 계보".to_string())).unwrap();
        let source_icon = import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![ImportImageFilePayload {
                original_filename: "original.png".to_string(),
                bytes: png_bytes([20, 40, 220, 255]),
            }],
        )
        .unwrap()
        .imported_icons
        .into_iter()
        .next()
        .unwrap();

        let first = import_local_ai_candidate(
            &mut connection,
            &paths,
            &collection.id,
            ImportAiCandidatePayload {
                icon_id: source_icon.id.clone(),
                service_surface: "gemini_web".to_string(),
                file: ImportImageFilePayload {
                    original_filename: "first.png".to_string(),
                    bytes: png_bytes([230, 80, 30, 255]),
                },
            },
        )
        .unwrap();
        let first_candidate_id = first.candidates[0].id.clone();
        let first_active = activate_ai_candidate(
            &mut connection,
            &paths,
            &collection.id,
            ActivateAiCandidatePayload {
                icon_id: source_icon.id.clone(),
                candidate_id: first_candidate_id.clone(),
                expected_revision: first.visual_source.activation_revision,
                normalization: Default::default(),
                expected_preview_signature: None,
            },
        )
        .unwrap();
        let second = import_local_ai_candidate(
            &mut connection,
            &paths,
            &collection.id,
            ImportAiCandidatePayload {
                icon_id: source_icon.id.clone(),
                service_surface: "novelai_web".to_string(),
                file: ImportImageFilePayload {
                    original_filename: "old-lineage.png".to_string(),
                    bytes: png_bytes([30, 210, 100, 255]),
                },
            },
        )
        .unwrap();
        let old_lineage_candidate_id = second
            .candidates
            .iter()
            .find(|candidate| candidate.source.original_filename == "old-lineage.png")
            .unwrap()
            .id
            .clone();
        assert_eq!(
            second.visual_source.activation_revision,
            first_active.review_state.visual_source.activation_revision
        );

        let replaced = replace_icon_source(
            &mut connection,
            &paths,
            &collection.id,
            &source_icon.id,
            ImportImageFilePayload {
                original_filename: "replacement.png".to_string(),
                bytes: png_bytes([90, 70, 180, 255]),
            },
        )
        .unwrap();
        let after_replace =
            get_ai_review_state(&connection, &collection.id, &source_icon.id).unwrap();
        let old_candidate = after_replace
            .candidates
            .iter()
            .find(|candidate| candidate.id == old_lineage_candidate_id)
            .unwrap();
        assert!(old_candidate.is_stale);
        assert!(!old_candidate.is_materialized);

        let current = import_local_ai_candidate(
            &mut connection,
            &paths,
            &collection.id,
            ImportAiCandidatePayload {
                icon_id: source_icon.id.clone(),
                service_surface: "gemini_web".to_string(),
                file: ImportImageFilePayload {
                    original_filename: "current-lineage.png".to_string(),
                    bytes: png_bytes([240, 190, 20, 255]),
                },
            },
        )
        .unwrap();
        let current_candidate_id = current
            .candidates
            .iter()
            .find(|candidate| candidate.source.original_filename == "current-lineage.png")
            .unwrap()
            .id
            .clone();
        let source_before = activate_ai_candidate(
            &mut connection,
            &paths,
            &collection.id,
            ActivateAiCandidatePayload {
                icon_id: source_icon.id.clone(),
                candidate_id: current_candidate_id.clone(),
                expected_revision: current.visual_source.activation_revision,
                normalization: Default::default(),
                expected_preview_signature: None,
            },
        )
        .unwrap();
        let request_count_before: i64 = connection
            .query_row("SELECT COUNT(*) FROM ai_requests", [], |row| row.get(0))
            .unwrap();
        let candidate_count_before: i64 = connection
            .query_row("SELECT COUNT(*) FROM ai_candidates", [], |row| row.get(0))
            .unwrap();
        let source_version_ids = source_before
            .review_state
            .versions
            .iter()
            .map(|version| version.id.clone())
            .collect::<Vec<_>>();

        let created = create_ai_icon_root(
            &mut connection,
            &paths,
            &collection.id,
            CreateAiIconRootPayload {
                icon_id: source_icon.id.clone(),
                candidate_id: old_lineage_candidate_id.clone(),
                expected_revision: source_before.review_state.visual_source.activation_revision,
                normalization: Default::default(),
                expected_preview_signature: None,
            },
        )
        .unwrap();
        let created_review_state =
            get_ai_review_state(&connection, &collection.id, &created.created_icon.id).unwrap();

        assert_ne!(created.created_icon.id, source_icon.id);
        let first_created_id = created.created_icon.id.clone();
        assert_eq!(created.created_icon_usage.created_icon_count, 1);
        assert_eq!(
            created
                .created_icon_usage
                .latest_created_icon
                .as_ref()
                .map(|icon| icon.id.as_str()),
            Some(first_created_id.as_str())
        );
        assert_eq!(
            created
                .source_review_state
                .visual_source
                .active_candidate_id
                .as_deref(),
            Some(current_candidate_id.as_str())
        );
        assert_eq!(
            created
                .source_review_state
                .visual_source
                .activation_revision,
            source_before.review_state.visual_source.activation_revision
        );
        assert_eq!(created.created_icon.source_file_id, replaced.source_file_id);
        assert_eq!(created.created_icon.readiness, "working");
        assert_eq!(created.created_icon.icon_kind, "image");
        assert!(created
            .created_icon
            .pieces
            .iter()
            .all(|piece| piece.alt_text.is_empty()));
        let preview = created.created_icon.current_preview_url.as_ref().unwrap();
        assert!(preview.contains(&created.created_icon.id));
        assert!(std::path::Path::new(preview).is_file());
        assert!(created.created_icon.pieces.iter().all(|piece| {
            piece.generated_preview_url.as_deref().is_some_and(|path| {
                path.contains(&created.created_icon.id) && std::path::Path::new(path).is_file()
            })
        }));
        assert_eq!(
            created_review_state
                .visual_source
                .active_candidate_id
                .as_deref(),
            Some(old_lineage_candidate_id.as_str())
        );
        assert_eq!(
            created_review_state.versions.len(),
            source_version_ids.len() + 1
        );
        assert!(created_review_state
            .versions
            .iter()
            .all(|version| !source_version_ids.contains(&version.id)));

        let (apply_kind, parent_candidate_id): (String, Option<String>) = connection
            .query_row(
                "SELECT child.apply_kind, parent.candidate_id
                 FROM icon_ai_versions child
                 LEFT JOIN icon_ai_versions parent
                   ON parent.icon_id = child.icon_id
                  AND parent.id = child.parent_version_id
                 WHERE child.icon_id = ?1
                   AND child.candidate_id = ?2
                   AND child.apply_kind = 'new_icon_root'",
                rusqlite::params![created.created_icon.id, old_lineage_candidate_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(apply_kind, "new_icon_root");
        assert_eq!(
            parent_candidate_id.as_deref(),
            Some(current_candidate_id.as_str())
        );

        let (source_lineage, target_lineage): (String, String) = connection
            .query_row(
                "SELECT source.original_lineage_id, target.original_lineage_id
                 FROM icons source
                 JOIN icons target ON target.id = ?1
                 WHERE source.id = ?2",
                rusqlite::params![created.created_icon.id, source_icon.id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_ne!(source_lineage, target_lineage);
        let source_after =
            get_ai_review_state(&connection, &collection.id, &source_icon.id).unwrap();
        assert_eq!(
            source_after.visual_source.active_candidate_id,
            source_before.review_state.visual_source.active_candidate_id
        );
        assert_eq!(
            source_after.visual_source.activation_revision,
            source_before.review_state.visual_source.activation_revision
        );
        let request_count_after: i64 = connection
            .query_row("SELECT COUNT(*) FROM ai_requests", [], |row| row.get(0))
            .unwrap();
        let candidate_count_after: i64 = connection
            .query_row("SELECT COUNT(*) FROM ai_candidates", [], |row| row.get(0))
            .unwrap();
        assert_eq!(request_count_after, request_count_before);
        assert_eq!(candidate_count_after, candidate_count_before);

        let second_created = create_ai_icon_root(
            &mut connection,
            &paths,
            &collection.id,
            CreateAiIconRootPayload {
                icon_id: source_icon.id.clone(),
                candidate_id: old_lineage_candidate_id.clone(),
                expected_revision: source_before.review_state.visual_source.activation_revision,
                normalization: Default::default(),
                expected_preview_signature: None,
            },
        )
        .unwrap();
        let second_created_id = second_created.created_icon.id.clone();
        assert_eq!(second_created.created_icon_usage.created_icon_count, 2);
        assert_eq!(
            second_created
                .created_icon_usage
                .latest_created_icon
                .as_ref()
                .map(|icon| icon.id.as_str()),
            Some(second_created_id.as_str())
        );

        let ordinary_duplicate =
            duplicate_icon(&mut connection, &paths, &collection.id, &second_created_id).unwrap();
        let duplicate_direct_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM ai_icon_root_creations WHERE icon_id = ?1",
                [&ordinary_duplicate.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(duplicate_direct_count, 0);
        let source_after_duplicate =
            get_ai_review_state(&connection, &collection.id, &source_icon.id).unwrap();
        let usage_after_duplicate = source_after_duplicate
            .candidates
            .iter()
            .find(|candidate| candidate.id == old_lineage_candidate_id)
            .unwrap()
            .created_icon_usage
            .clone();
        assert_eq!(usage_after_duplicate.created_icon_count, 2);
        assert_eq!(
            usage_after_duplicate
                .latest_created_icon
                .as_ref()
                .map(|icon| icon.id.as_str()),
            Some(second_created_id.as_str())
        );

        let duplicated_collection =
            duplicate_collection(&mut connection, &paths, &collection.id).unwrap();
        let cloned_collection_direct_count: i64 = connection
            .query_row(
                "SELECT COUNT(*)
                 FROM ai_icon_root_creations creation
                 JOIN icons icon ON icon.id = creation.icon_id
                 WHERE icon.collection_id = ?1",
                [&duplicated_collection.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(cloned_collection_direct_count, 0);

        delete_icons(
            &mut connection,
            &collection.id,
            vec![second_created_id.clone()],
        )
        .unwrap();
        let source_after_latest_delete =
            get_ai_review_state(&connection, &collection.id, &source_icon.id).unwrap();
        let usage_after_latest_delete = source_after_latest_delete
            .candidates
            .iter()
            .find(|candidate| candidate.id == old_lineage_candidate_id)
            .unwrap()
            .created_icon_usage
            .clone();
        assert_eq!(usage_after_latest_delete.created_icon_count, 1);
        assert_eq!(
            usage_after_latest_delete
                .latest_created_icon
                .as_ref()
                .map(|icon| icon.id.as_str()),
            Some(first_created_id.as_str())
        );

        delete_icons(&mut connection, &collection.id, vec![first_created_id]).unwrap();
        let source_after_all_direct_deletes =
            get_ai_review_state(&connection, &collection.id, &source_icon.id).unwrap();
        let usage_after_all_direct_deletes = source_after_all_direct_deletes
            .candidates
            .iter()
            .find(|candidate| candidate.id == old_lineage_candidate_id)
            .unwrap()
            .created_icon_usage
            .clone();
        assert_eq!(usage_after_all_direct_deletes.created_icon_count, 0);
        assert!(usage_after_all_direct_deletes.latest_created_icon.is_none());
    }
    fs::remove_dir_all(paths.root).unwrap();
}

#[test]
fn new_icon_root_db_failure_removes_partial_icon_and_promoted_files() {
    let paths = temp_paths();
    {
        let mut connection = open_database(&paths.database_path).unwrap();
        let collection =
            create_collection(&mut connection, Some("AI 새 아이콘 보상".to_string())).unwrap();
        let source_icon = import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![ImportImageFilePayload {
                original_filename: "original.png".to_string(),
                bytes: png_bytes([20, 40, 220, 255]),
            }],
        )
        .unwrap()
        .imported_icons
        .into_iter()
        .next()
        .unwrap();
        let review = import_local_ai_candidate(
            &mut connection,
            &paths,
            &collection.id,
            ImportAiCandidatePayload {
                icon_id: source_icon.id.clone(),
                service_surface: "gemini_web".to_string(),
                file: ImportImageFilePayload {
                    original_filename: "candidate.png".to_string(),
                    bytes: png_bytes([230, 80, 30, 255]),
                },
            },
        )
        .unwrap();
        let candidate_id = review.candidates[0].id.clone();
        let icon_count_before: i64 = connection
            .query_row("SELECT COUNT(*) FROM icons", [], |row| row.get(0))
            .unwrap();
        let version_count_before: i64 = connection
            .query_row("SELECT COUNT(*) FROM icon_ai_versions", [], |row| {
                row.get(0)
            })
            .unwrap();
        let creation_count_before: i64 = connection
            .query_row("SELECT COUNT(*) FROM ai_icon_root_creations", [], |row| {
                row.get(0)
            })
            .unwrap();
        let staging_entries_before = managed_entry_count(&paths.ai_activation_staging_dir);
        let preview_entries_before = managed_entry_count(&paths.ai_activation_previews_dir);
        connection
            .execute_batch(
                "CREATE TEMP TRIGGER fail_ai_new_icon_preview
                 BEFORE UPDATE OF current_preview_path ON icons
                 BEGIN
                   SELECT RAISE(ABORT, 'injected new icon failure');
                 END;",
            )
            .unwrap();

        let error = create_ai_icon_root(
            &mut connection,
            &paths,
            &collection.id,
            CreateAiIconRootPayload {
                icon_id: source_icon.id.clone(),
                candidate_id,
                expected_revision: review.visual_source.activation_revision,
                normalization: Default::default(),
                expected_preview_signature: None,
            },
        )
        .unwrap_err();
        assert!(!error.message.is_empty());
        let icon_count_after: i64 = connection
            .query_row("SELECT COUNT(*) FROM icons", [], |row| row.get(0))
            .unwrap();
        let version_count_after: i64 = connection
            .query_row("SELECT COUNT(*) FROM icon_ai_versions", [], |row| {
                row.get(0)
            })
            .unwrap();
        let creation_count_after: i64 = connection
            .query_row("SELECT COUNT(*) FROM ai_icon_root_creations", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(icon_count_after, icon_count_before);
        assert_eq!(version_count_after, version_count_before);
        assert_eq!(creation_count_after, creation_count_before);
        assert_eq!(
            managed_entry_count(&paths.ai_activation_staging_dir),
            staging_entries_before
        );
        assert_eq!(
            managed_entry_count(&paths.ai_activation_previews_dir),
            preview_entries_before
        );
        get_ai_review_state(&connection, &collection.id, &source_icon.id).unwrap();
    }
    fs::remove_dir_all(paths.root).unwrap();
}

fn managed_entry_count(root: &std::path::Path) -> usize {
    if !root.is_dir() {
        return 0;
    }
    fs::read_dir(root)
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            1 + if path.is_dir() {
                managed_entry_count(&path)
            } else {
                0
            }
        })
        .sum()
}
#[test]
fn new_icon_root_previous_and_original_restore_survive_restart() {
    let paths = temp_paths();
    let (
        collection_id,
        source_icon_id,
        target_icon_id,
        target_original_source_id,
        previous_version_id,
        previous_candidate_id,
        active_candidate_id,
    ) = {
        let mut connection = open_database(&paths.database_path).unwrap();
        let collection =
            create_collection(&mut connection, Some("AI 새 아이콘 재시작".to_string())).unwrap();
        let source_icon = import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![ImportImageFilePayload {
                original_filename: "original.png".to_string(),
                bytes: png_bytes([20, 40, 220, 255]),
            }],
        )
        .unwrap()
        .imported_icons
        .into_iter()
        .next()
        .unwrap();
        let first = import_local_ai_candidate(
            &mut connection,
            &paths,
            &collection.id,
            ImportAiCandidatePayload {
                icon_id: source_icon.id.clone(),
                service_surface: "gemini_web".to_string(),
                file: ImportImageFilePayload {
                    original_filename: "first.png".to_string(),
                    bytes: png_bytes([230, 80, 30, 255]),
                },
            },
        )
        .unwrap();
        let first_candidate_id = first.candidates[0].id.clone();
        let first_active = activate_ai_candidate(
            &mut connection,
            &paths,
            &collection.id,
            ActivateAiCandidatePayload {
                icon_id: source_icon.id.clone(),
                candidate_id: first_candidate_id.clone(),
                expected_revision: first.visual_source.activation_revision,
                normalization: Default::default(),
                expected_preview_signature: None,
            },
        )
        .unwrap();
        let second = import_local_ai_candidate(
            &mut connection,
            &paths,
            &collection.id,
            ImportAiCandidatePayload {
                icon_id: source_icon.id.clone(),
                service_surface: "novelai_web".to_string(),
                file: ImportImageFilePayload {
                    original_filename: "second.png".to_string(),
                    bytes: png_bytes([30, 210, 100, 255]),
                },
            },
        )
        .unwrap();
        let second_candidate_id = second
            .candidates
            .iter()
            .find(|candidate| candidate.source.original_filename == "second.png")
            .unwrap()
            .id
            .clone();
        let created = create_ai_icon_root(
            &mut connection,
            &paths,
            &collection.id,
            CreateAiIconRootPayload {
                icon_id: source_icon.id.clone(),
                candidate_id: second_candidate_id.clone(),
                expected_revision: first_active.review_state.visual_source.activation_revision,
                normalization: Default::default(),
                expected_preview_signature: None,
            },
        )
        .unwrap();
        let created_review_state =
            get_ai_review_state(&connection, &collection.id, &created.created_icon.id).unwrap();
        let previous_version_id: String = connection
            .query_row(
                "SELECT parent_version_id
                 FROM icon_ai_versions
                 WHERE icon_id = ?1
                   AND candidate_id = ?2
                   AND apply_kind = 'new_icon_root'",
                rusqlite::params![created.created_icon.id, second_candidate_id],
                |row| row.get(0),
            )
            .unwrap();
        (
            collection.id,
            source_icon.id,
            created.created_icon.id,
            created_review_state.visual_source.original_source.id,
            previous_version_id,
            first_candidate_id,
            second_candidate_id,
        )
    };

    {
        let mut connection = open_database(&paths.database_path).unwrap();
        let persisted = get_ai_review_state(&connection, &collection_id, &target_icon_id).unwrap();
        assert_eq!(
            persisted.visual_source.active_candidate_id.as_deref(),
            Some(active_candidate_id.as_str())
        );
        let previous = restore_ai_version(
            &mut connection,
            &paths,
            &collection_id,
            RestoreAiVersionPayload {
                icon_id: target_icon_id.clone(),
                version_id: Some(previous_version_id.clone()),
                expected_revision: persisted.visual_source.activation_revision,
            },
        )
        .unwrap();
        assert_eq!(
            previous
                .review_state
                .visual_source
                .active_candidate_id
                .as_deref(),
            Some(previous_candidate_id.as_str())
        );
        let original = restore_ai_version(
            &mut connection,
            &paths,
            &collection_id,
            RestoreAiVersionPayload {
                icon_id: target_icon_id.clone(),
                version_id: None,
                expected_revision: previous.review_state.visual_source.activation_revision,
            },
        )
        .unwrap();
        assert_eq!(original.review_state.visual_source.active_version_id, None);
        assert_eq!(
            original
                .review_state
                .visual_source
                .effective_render_source
                .id,
            target_original_source_id
        );
        let previous_again = restore_ai_version(
            &mut connection,
            &paths,
            &collection_id,
            RestoreAiVersionPayload {
                icon_id: target_icon_id,
                version_id: Some(previous_version_id),
                expected_revision: original.review_state.visual_source.activation_revision,
            },
        )
        .unwrap();
        assert_eq!(
            previous_again
                .review_state
                .visual_source
                .active_candidate_id
                .as_deref(),
            Some(previous_candidate_id.as_str())
        );
        let source = get_ai_review_state(&connection, &collection_id, &source_icon_id).unwrap();
        assert_eq!(
            source.visual_source.active_candidate_id.as_deref(),
            Some(previous_candidate_id.as_str())
        );
    }
    fs::remove_dir_all(paths.root).unwrap();
}

#[test]
fn new_icon_root_render_limit_failure_rolls_back_order_and_all_artifacts() {
    let paths = temp_paths();
    {
        let mut connection = open_database(&paths.database_path).unwrap();
        let collection =
            create_collection(&mut connection, Some("AI 새 아이콘 렌더 실패".to_string())).unwrap();
        let imported = import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![
                ImportImageFilePayload {
                    original_filename: "source.png".to_string(),
                    bytes: png_bytes([20, 40, 220, 255]),
                },
                ImportImageFilePayload {
                    original_filename: "sibling.png".to_string(),
                    bytes: png_bytes([80, 200, 30, 255]),
                },
            ],
        )
        .unwrap()
        .imported_icons;
        let source_icon = imported[0].clone();
        let sibling_icon = imported[1].clone();
        let review = import_local_ai_candidate(
            &mut connection,
            &paths,
            &collection.id,
            ImportAiCandidatePayload {
                icon_id: source_icon.id.clone(),
                service_surface: "gemini_web".to_string(),
                file: ImportImageFilePayload {
                    original_filename: "candidate.png".to_string(),
                    bytes: png_bytes([230, 80, 30, 255]),
                },
            },
        )
        .unwrap();
        let orders_before: (i64, i64) = connection
            .query_row(
                "SELECT source.order_index, sibling.order_index
                 FROM icons source
                 JOIN icons sibling ON sibling.id = ?1
                 WHERE source.id = ?2",
                rusqlite::params![sibling_icon.id, source_icon.id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        let icon_count_before: i64 = connection
            .query_row("SELECT COUNT(*) FROM icons", [], |row| row.get(0))
            .unwrap();
        let state_count_before: i64 = connection
            .query_row("SELECT COUNT(*) FROM icon_ai_state", [], |row| row.get(0))
            .unwrap();
        let lineage_count_before: i64 = connection
            .query_row("SELECT COUNT(*) FROM icon_ai_lineages", [], |row| {
                row.get(0)
            })
            .unwrap();
        let staging_entries_before = managed_entry_count(&paths.ai_activation_staging_dir);
        let preview_entries_before = managed_entry_count(&paths.ai_activation_previews_dir);
        connection
            .execute(
                "UPDATE collections SET max_bytes = 1 WHERE id = ?1",
                [&collection.id],
            )
            .unwrap();

        let error = create_ai_icon_root(
            &mut connection,
            &paths,
            &collection.id,
            CreateAiIconRootPayload {
                icon_id: source_icon.id.clone(),
                candidate_id: review.candidates[0].id.clone(),
                expected_revision: review.visual_source.activation_revision,
                normalization: Default::default(),
                expected_preview_signature: None,
            },
        )
        .unwrap_err();
        assert_eq!(error.code, "validation");
        let orders_after: (i64, i64) = connection
            .query_row(
                "SELECT source.order_index, sibling.order_index
                 FROM icons source
                 JOIN icons sibling ON sibling.id = ?1
                 WHERE source.id = ?2",
                rusqlite::params![sibling_icon.id, source_icon.id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(orders_after, orders_before);
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM icons", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            icon_count_before
        );
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM icon_ai_state", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            state_count_before
        );
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM icon_ai_lineages", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            lineage_count_before
        );
        assert_eq!(
            managed_entry_count(&paths.ai_activation_staging_dir),
            staging_entries_before
        );
        assert_eq!(
            managed_entry_count(&paths.ai_activation_previews_dir),
            preview_entries_before
        );
        get_ai_review_state(&connection, &collection.id, &source_icon.id).unwrap();
    }
    fs::remove_dir_all(paths.root).unwrap();
}

fn try_create_directory_link(target: &std::path::Path, link: &std::path::Path) -> bool {
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_dir(target, link).is_ok()
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link).is_ok()
    }
    #[cfg(not(any(windows, unix)))]
    {
        let _ = (target, link);
        false
    }
}

fn remove_directory_link(link: &std::path::Path) {
    #[cfg(windows)]
    {
        let _ = fs::remove_dir(link);
    }
    #[cfg(unix)]
    {
        let _ = fs::remove_file(link);
    }
}

#[test]
fn new_icon_root_rejects_preview_parent_directory_links_without_external_writes() {
    let paths = temp_paths();
    let outside = std::env::temp_dir().join(format!(
        "pmtconcon-ai-outside-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&outside).unwrap();

    let mut connection = open_database(&paths.database_path).unwrap();
    let collection = create_collection(&mut connection, Some("AI 경로 격리".to_string())).unwrap();
    let source_icon = import_image_files(
        &mut connection,
        &paths,
        &collection.id,
        vec![ImportImageFilePayload {
            original_filename: "original.png".to_string(),
            bytes: png_bytes([20, 40, 220, 255]),
        }],
    )
    .unwrap()
    .imported_icons
    .into_iter()
    .next()
    .unwrap();
    let review = import_local_ai_candidate(
        &mut connection,
        &paths,
        &collection.id,
        ImportAiCandidatePayload {
            icon_id: source_icon.id.clone(),
            service_surface: "gemini_web".to_string(),
            file: ImportImageFilePayload {
                original_filename: "candidate.png".to_string(),
                bytes: png_bytes([230, 80, 30, 255]),
            },
        },
    )
    .unwrap();

    let linked_collection_dir = paths.ai_activation_previews_dir.join(&collection.id);
    if !try_create_directory_link(&outside, &linked_collection_dir) {
        drop(connection);
        fs::remove_dir_all(paths.root).unwrap();
        fs::remove_dir_all(outside).unwrap();
        return;
    }
    let icon_count_before: i64 = connection
        .query_row("SELECT COUNT(*) FROM icons", [], |row| row.get(0))
        .unwrap();
    let source_order_before: i64 = connection
        .query_row(
            "SELECT order_index FROM icons WHERE id = ?1",
            [&source_icon.id],
            |row| row.get(0),
        )
        .unwrap();
    let outside_entries_before = managed_entry_count(&outside);

    let error = create_ai_icon_root(
        &mut connection,
        &paths,
        &collection.id,
        CreateAiIconRootPayload {
            icon_id: source_icon.id.clone(),
            candidate_id: review.candidates[0].id.clone(),
            expected_revision: review.visual_source.activation_revision,
            normalization: Default::default(),
            expected_preview_signature: None,
        },
    )
    .unwrap_err();
    assert_eq!(error.code, "ai_new_icon_path");
    assert_eq!(
        connection
            .query_row("SELECT COUNT(*) FROM icons", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        icon_count_before
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT order_index FROM icons WHERE id = ?1",
                [&source_icon.id],
                |row| row.get::<_, i64>(0),
            )
            .unwrap(),
        source_order_before
    );
    assert_eq!(managed_entry_count(&outside), outside_entries_before);
    assert_eq!(managed_entry_count(&paths.ai_activation_staging_dir), 0);

    drop(connection);
    remove_directory_link(&linked_collection_dir);
    fs::remove_dir_all(paths.root).unwrap();
    fs::remove_dir_all(outside).unwrap();
}

#[test]
fn current_icon_activation_rejects_preview_parent_directory_links_without_external_writes() {
    let paths = temp_paths();
    let outside = std::env::temp_dir().join(format!(
        "pmtconcon-ai-activation-outside-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&outside).unwrap();

    let mut connection = open_database(&paths.database_path).unwrap();
    let collection =
        create_collection(&mut connection, Some("AI 적용 경로 격리".to_string())).unwrap();
    let source_icon = import_image_files(
        &mut connection,
        &paths,
        &collection.id,
        vec![ImportImageFilePayload {
            original_filename: "original.png".to_string(),
            bytes: png_bytes([20, 40, 220, 255]),
        }],
    )
    .unwrap()
    .imported_icons
    .into_iter()
    .next()
    .unwrap();
    let review = import_local_ai_candidate(
        &mut connection,
        &paths,
        &collection.id,
        ImportAiCandidatePayload {
            icon_id: source_icon.id.clone(),
            service_surface: "gemini_web".to_string(),
            file: ImportImageFilePayload {
                original_filename: "candidate.png".to_string(),
                bytes: png_bytes_with_dimensions(320, 120, [230, 80, 30, 180]),
            },
        },
    )
    .unwrap();

    let linked_collection_dir = paths.ai_activation_previews_dir.join(&collection.id);
    if !try_create_directory_link(&outside, &linked_collection_dir) {
        drop(connection);
        fs::remove_dir_all(paths.root).unwrap();
        fs::remove_dir_all(outside).unwrap();
        return;
    }
    let version_count_before: i64 = connection
        .query_row("SELECT COUNT(*) FROM icon_ai_versions", [], |row| {
            row.get(0)
        })
        .unwrap();
    let source_count_before: i64 = connection
        .query_row("SELECT COUNT(*) FROM source_files", [], |row| row.get(0))
        .unwrap();
    let outside_entries_before = managed_entry_count(&outside);

    let error = activate_ai_candidate(
        &mut connection,
        &paths,
        &collection.id,
        ActivateAiCandidatePayload {
            icon_id: source_icon.id.clone(),
            candidate_id: review.candidates[0].id.clone(),
            expected_revision: review.visual_source.activation_revision,
            normalization: Default::default(),
            expected_preview_signature: None,
        },
    )
    .unwrap_err();

    assert_eq!(error.code, "ai_managed_artifact_path");
    let after = get_ai_review_state(&connection, &collection.id, &source_icon.id).unwrap();
    assert_eq!(
        after.visual_source.activation_revision,
        review.visual_source.activation_revision
    );
    assert!(after.visual_source.active_version_id.is_none());
    assert_eq!(
        connection
            .query_row("SELECT COUNT(*) FROM icon_ai_versions", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        version_count_before
    );
    assert_eq!(
        connection
            .query_row("SELECT COUNT(*) FROM source_files", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        source_count_before
    );
    assert_eq!(managed_entry_count(&outside), outside_entries_before);
    assert_eq!(managed_entry_count(&paths.ai_activation_staging_dir), 0);

    drop(connection);
    remove_directory_link(&linked_collection_dir);
    fs::remove_dir_all(paths.root).unwrap();
    fs::remove_dir_all(outside).unwrap();
}
#[test]
fn new_icon_root_rejects_non_base_source_candidate_versions() {
    let paths = temp_paths();
    {
        let mut connection = open_database(&paths.database_path).unwrap();
        let collection =
            create_collection(&mut connection, Some("AI 입력 단계".to_string())).unwrap();
        let source_icon = import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![ImportImageFilePayload {
                original_filename: "original.png".to_string(),
                bytes: png_bytes([20, 40, 220, 255]),
            }],
        )
        .unwrap()
        .imported_icons
        .into_iter()
        .next()
        .unwrap();
        let review = import_local_ai_candidate(
            &mut connection,
            &paths,
            &collection.id,
            ImportAiCandidatePayload {
                icon_id: source_icon.id.clone(),
                service_surface: "gemini_web".to_string(),
                file: ImportImageFilePayload {
                    original_filename: "rendered.png".to_string(),
                    bytes: png_bytes([230, 80, 30, 255]),
                },
            },
        )
        .unwrap();
        let candidate_id = review.candidates[0].id.clone();
        let active = activate_ai_candidate(
            &mut connection,
            &paths,
            &collection.id,
            ActivateAiCandidatePayload {
                icon_id: source_icon.id.clone(),
                candidate_id: candidate_id.clone(),
                expected_revision: review.visual_source.activation_revision,
                normalization: Default::default(),
                expected_preview_signature: None,
            },
        )
        .unwrap();
        connection
            .execute(
                "INSERT INTO icon_ai_versions (
                   id, icon_id, candidate_id, base_original_source_file_id,
                   base_original_lineage_id, base_original_lineage_generation,
                   parent_version_id, effective_source_file_id, input_stage, apply_kind,
                   provider_native_width, provider_native_height,
                   target_canvas_width, target_canvas_height,
                   normalization_recipe_json, normalization_recipe_hash,
                   canvas_kind, animation_kind, payload_input_signature, created_at
                 )
                 SELECT
                   'ai_version_rendered_viewport_fixture', icon_id, candidate_id,
                   base_original_source_file_id, base_original_lineage_id,
                   base_original_lineage_generation, id, effective_source_file_id,
                   'rendered_viewport', 'new_icon_root', provider_native_width,
                   provider_native_height, target_canvas_width, target_canvas_height,
                   normalization_recipe_json, normalization_recipe_hash,
                   canvas_kind, animation_kind, payload_input_signature,
                   strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                 FROM icon_ai_versions
                 WHERE icon_id = ?1 AND candidate_id = ?2
                 LIMIT 1",
                rusqlite::params![source_icon.id, candidate_id],
            )
            .unwrap();
        let icon_count_before: i64 = connection
            .query_row("SELECT COUNT(*) FROM icons", [], |row| row.get(0))
            .unwrap();

        let error = create_ai_icon_root(
            &mut connection,
            &paths,
            &collection.id,
            CreateAiIconRootPayload {
                icon_id: source_icon.id.clone(),
                candidate_id,
                expected_revision: active.review_state.visual_source.activation_revision,
                normalization: Default::default(),
                expected_preview_signature: None,
            },
        )
        .unwrap_err();
        assert_eq!(error.code, "ai_new_icon_input_stage");
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM icons", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            icon_count_before
        );
        assert_eq!(managed_entry_count(&paths.ai_activation_staging_dir), 0);
    }
    fs::remove_dir_all(paths.root).unwrap();
}

#[test]
fn arbitrary_size_candidate_preview_apply_and_original_restore_preserve_raw_source() {
    let paths = temp_paths();
    {
        let mut connection = open_database(&paths.database_path).unwrap();
        let collection =
            create_collection(&mut connection, Some("AI 정규화 통합".to_string())).unwrap();
        let icon = import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![ImportImageFilePayload {
                original_filename: "original.png".to_string(),
                bytes: png_bytes([20, 40, 220, 255]),
            }],
        )
        .unwrap()
        .imported_icons
        .into_iter()
        .next()
        .unwrap();
        let original_source_id = icon.source_file_id.clone();
        let candidate_bytes = png_bytes_with_dimensions(13, 5, [230, 80, 30, 180]);
        let expected_raw_sha = format!("{:x}", Sha256::digest(&candidate_bytes));
        let review = import_local_ai_candidate(
            &mut connection,
            &paths,
            &collection.id,
            ImportAiCandidatePayload {
                icon_id: icon.id.clone(),
                service_surface: "gemini_web".to_string(),
                file: ImportImageFilePayload {
                    original_filename: "wide-candidate.png".to_string(),
                    bytes: candidate_bytes.clone(),
                },
            },
        )
        .unwrap();
        let candidate = review
            .candidates
            .iter()
            .find(|candidate| candidate.source.original_filename == "wide-candidate.png")
            .unwrap();
        let candidate_id = candidate.id.clone();
        let raw_source_id = candidate.source.id.clone();
        let raw_source_path = candidate.source.original_image_url.clone();
        assert_eq!((candidate.source.width, candidate.source.height), (13, 5));
        assert_eq!(candidate.source.sha256, expected_raw_sha);
        assert_eq!(fs::read(&raw_source_path).unwrap(), candidate_bytes);

        let normalization = AiNormalizationOptionsPayload {
            mode: "contain_pad".to_string(),
            alignment: "center".to_string(),
            resize_filter: "nearest".to_string(),
            pad_rgba: [0, 0, 0, 0],
        };
        let preview_once = preview_ai_candidate_normalization(
            &connection,
            &paths,
            &collection.id,
            PreviewAiCandidateNormalizationPayload {
                icon_id: icon.id.clone(),
                candidate_id: candidate_id.clone(),
                expected_revision: review.visual_source.activation_revision,
                normalization: normalization.clone(),
            },
        )
        .unwrap();
        let preview_twice = preview_ai_candidate_normalization(
            &connection,
            &paths,
            &collection.id,
            PreviewAiCandidateNormalizationPayload {
                icon_id: icon.id.clone(),
                candidate_id: candidate_id.clone(),
                expected_revision: review.visual_source.activation_revision,
                normalization: normalization.clone(),
            },
        )
        .unwrap();

        assert_eq!(
            (
                preview_once.target_canvas_width,
                preview_once.target_canvas_height
            ),
            (8, 8)
        );
        let decoded_final_dimensions =
            image::image_dimensions(&preview_once.final_preview_path).unwrap();
        assert_eq!(
            (
                preview_once.final_render_width,
                preview_once.final_render_height
            ),
            (
                i64::from(decoded_final_dimensions.0),
                i64::from(decoded_final_dimensions.1)
            )
        );
        assert_eq!(
            (preview_once.piece_width, preview_once.piece_height),
            (200, 200)
        );
        assert_ne!(
            (
                preview_once.final_render_width,
                preview_once.final_render_height
            ),
            (
                preview_once.target_canvas_width,
                preview_once.target_canvas_height
            )
        );
        assert_eq!(preview_once.geometry.kind, "contain_pad");
        assert_eq!(
            (
                preview_once.geometry.resized_width,
                preview_once.geometry.resized_height
            ),
            (8, 3)
        );
        assert_eq!(preview_once.raw_source.id, raw_source_id);
        assert_eq!(preview_once.raw_source.original_image_url, raw_source_path);
        assert_eq!(preview_once.raw_source.sha256, expected_raw_sha);
        assert_eq!(
            (
                preview_once.raw_source.width,
                preview_once.raw_source.height
            ),
            (13, 5)
        );
        assert_eq!(
            preview_once.preview_signature,
            preview_twice.preview_signature
        );
        assert_eq!(
            preview_once.normalization_recipe_hash,
            preview_twice.normalization_recipe_hash
        );
        let normalized = image::open(&preview_once.normalized_preview_path)
            .unwrap()
            .to_rgba8();
        assert_eq!(normalized.dimensions(), (8, 8));
        assert!(std::path::Path::new(&preview_once.final_preview_path).is_file());

        let applied = activate_ai_candidate(
            &mut connection,
            &paths,
            &collection.id,
            ActivateAiCandidatePayload {
                icon_id: icon.id.clone(),
                candidate_id: candidate_id.clone(),
                expected_revision: review.visual_source.activation_revision,
                normalization,
                expected_preview_signature: Some(preview_once.preview_signature.clone()),
            },
        )
        .unwrap();
        let version = applied
            .review_state
            .versions
            .iter()
            .find(|version| version.candidate_id == candidate_id)
            .unwrap();
        assert!(version.is_active);
        assert_eq!(
            version.normalization_recipe_hash,
            preview_once.normalization_recipe_hash
        );
        let summary = version.normalization_summary.as_ref().unwrap();
        assert_eq!(summary.kind, "contain_pad");
        assert_eq!(summary.mode.as_deref(), Some("contain_pad"));
        assert_eq!(summary.alignment.as_deref(), Some("center"));
        assert_eq!(summary.resize_filter.as_deref(), Some("nearest"));
        assert_eq!(
            (summary.target_canvas_width, summary.target_canvas_height),
            (8, 8)
        );
        assert_eq!((version.source.width, version.source.height), (8, 8));
        assert_eq!(version.source.original_extension, "png");
        assert_ne!(version.source.id, raw_source_id);
        assert_ne!(version.source.original_image_url, raw_source_path);

        let (
            provider_width,
            provider_height,
            target_width,
            target_height,
            recipe_json,
            recipe_hash,
        ): (i64, i64, i64, i64, String, String) = connection
            .query_row(
                "SELECT provider_native_width, provider_native_height,
                        target_canvas_width, target_canvas_height,
                        normalization_recipe_json, normalization_recipe_hash
                 FROM icon_ai_versions
                 WHERE id = ?1",
                [&version.id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!((provider_width, provider_height), (13, 5));
        assert_eq!((target_width, target_height), (8, 8));
        assert_eq!(recipe_hash, preview_once.normalization_recipe_hash);
        let recipe: serde_json::Value = serde_json::from_str(&recipe_json).unwrap();
        assert_eq!(recipe["kind"], "contain_pad");
        assert_eq!(recipe["providerNativeWidth"], 13);
        assert_eq!(recipe["providerNativeHeight"], 5);
        assert_eq!(recipe["targetCanvasWidth"], 8);
        assert_eq!(recipe["targetCanvasHeight"], 8);

        let persisted_candidate = applied
            .review_state
            .candidates
            .iter()
            .find(|candidate| candidate.id == candidate_id)
            .unwrap();
        assert_eq!(persisted_candidate.source.id, raw_source_id);
        assert_eq!(
            persisted_candidate.source.original_image_url,
            raw_source_path
        );
        assert_eq!(persisted_candidate.source.sha256, expected_raw_sha);
        assert_eq!(fs::read(&raw_source_path).unwrap(), candidate_bytes);

        let restored = restore_ai_version(
            &mut connection,
            &paths,
            &collection.id,
            RestoreAiVersionPayload {
                icon_id: icon.id,
                version_id: None,
                expected_revision: applied.review_state.visual_source.activation_revision,
            },
        )
        .unwrap();
        assert_eq!(restored.review_state.visual_source.active_version_id, None);
        assert_eq!(
            restored
                .review_state
                .visual_source
                .effective_render_source
                .id,
            original_source_id
        );
        let restored_candidate = restored
            .review_state
            .candidates
            .iter()
            .find(|candidate| candidate.id == candidate_id)
            .unwrap();
        assert_eq!(restored_candidate.source.id, raw_source_id);
        assert_eq!(restored_candidate.source.sha256, expected_raw_sha);
        assert_eq!(
            restored_candidate.source.original_image_url,
            raw_source_path
        );
        assert_eq!(fs::read(&raw_source_path).unwrap(), candidate_bytes);
    }
    fs::remove_dir_all(paths.root).unwrap();
}

#[test]
fn applying_with_a_stale_normalization_preview_signature_is_rejected() {
    let paths = temp_paths();
    {
        let mut connection = open_database(&paths.database_path).unwrap();
        let collection =
            create_collection(&mut connection, Some("AI 미리보기 충돌".to_string())).unwrap();
        let icon = import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![ImportImageFilePayload {
                original_filename: "original.png".to_string(),
                bytes: png_bytes([20, 40, 220, 255]),
            }],
        )
        .unwrap()
        .imported_icons
        .into_iter()
        .next()
        .unwrap();
        let review = import_local_ai_candidate(
            &mut connection,
            &paths,
            &collection.id,
            ImportAiCandidatePayload {
                icon_id: icon.id.clone(),
                service_surface: "novelai_web".to_string(),
                file: ImportImageFilePayload {
                    original_filename: "candidate.png".to_string(),
                    bytes: png_bytes_with_dimensions(13, 5, [230, 80, 30, 255]),
                },
            },
        )
        .unwrap();
        let candidate_id = review.candidates[0].id.clone();
        let preview = preview_ai_candidate_normalization(
            &connection,
            &paths,
            &collection.id,
            PreviewAiCandidateNormalizationPayload {
                icon_id: icon.id.clone(),
                candidate_id: candidate_id.clone(),
                expected_revision: review.visual_source.activation_revision,
                normalization: AiNormalizationOptionsPayload::default(),
            },
        )
        .unwrap();
        let cover = AiNormalizationOptionsPayload {
            mode: "cover_crop".to_string(),
            alignment: "center".to_string(),
            resize_filter: "lanczos3".to_string(),
            pad_rgba: [0, 0, 0, 0],
        };

        let error = activate_ai_candidate(
            &mut connection,
            &paths,
            &collection.id,
            ActivateAiCandidatePayload {
                icon_id: icon.id.clone(),
                candidate_id,
                expected_revision: review.visual_source.activation_revision,
                normalization: cover,
                expected_preview_signature: Some(preview.preview_signature),
            },
        )
        .unwrap_err();
        assert_eq!(error.code, "ai_normalization_preview_stale");
        let unchanged = get_ai_review_state(&connection, &collection.id, &icon.id).unwrap();
        assert_eq!(unchanged.visual_source.active_version_id, None);
        assert!(unchanged.versions.is_empty());
    }
    fs::remove_dir_all(paths.root).unwrap();
}

#[test]
fn cover_crop_preview_can_create_an_independent_ai_icon_root() {
    let paths = temp_paths();
    {
        let mut connection = open_database(&paths.database_path).unwrap();
        let collection =
            create_collection(&mut connection, Some("AI 새 아이콘 정규화".to_string())).unwrap();
        let source_icon = import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![ImportImageFilePayload {
                original_filename: "original.png".to_string(),
                bytes: png_bytes([20, 40, 220, 255]),
            }],
        )
        .unwrap()
        .imported_icons
        .into_iter()
        .next()
        .unwrap();
        let candidate_bytes = png_bytes_with_dimensions(5, 13, [30, 210, 100, 255]);
        let review = import_local_ai_candidate(
            &mut connection,
            &paths,
            &collection.id,
            ImportAiCandidatePayload {
                icon_id: source_icon.id.clone(),
                service_surface: "gemini_web".to_string(),
                file: ImportImageFilePayload {
                    original_filename: "portrait-candidate.png".to_string(),
                    bytes: candidate_bytes.clone(),
                },
            },
        )
        .unwrap();
        let candidate_id = review.candidates[0].id.clone();
        let raw_path = review.candidates[0].source.original_image_url.clone();
        let raw_sha = review.candidates[0].source.sha256.clone();
        let normalization = AiNormalizationOptionsPayload {
            mode: "cover_crop".to_string(),
            alignment: "bottom".to_string(),
            resize_filter: "nearest".to_string(),
            pad_rgba: [0, 0, 0, 0],
        };
        let preview = preview_ai_candidate_normalization(
            &connection,
            &paths,
            &collection.id,
            PreviewAiCandidateNormalizationPayload {
                icon_id: source_icon.id.clone(),
                candidate_id: candidate_id.clone(),
                expected_revision: review.visual_source.activation_revision,
                normalization: normalization.clone(),
            },
        )
        .unwrap();
        assert!(preview.new_icon_compatibility.allowed);
        assert_eq!(preview.geometry.kind, "cover_crop");
        assert_eq!(
            (preview.target_canvas_width, preview.target_canvas_height),
            (8, 8)
        );

        let created = create_ai_icon_root(
            &mut connection,
            &paths,
            &collection.id,
            CreateAiIconRootPayload {
                icon_id: source_icon.id.clone(),
                candidate_id: candidate_id.clone(),
                expected_revision: review.visual_source.activation_revision,
                normalization,
                expected_preview_signature: Some(preview.preview_signature),
            },
        )
        .unwrap();
        let created_review_state =
            get_ai_review_state(&connection, &collection.id, &created.created_icon.id).unwrap();
        assert_ne!(created.created_icon.id, source_icon.id);
        assert_eq!(created.created_icon_usage.created_icon_count, 1);
        assert!(created
            .source_review_state
            .visual_source
            .active_candidate_id
            .is_none());
        assert_eq!(
            created_review_state
                .visual_source
                .active_candidate_id
                .as_deref(),
            Some(candidate_id.as_str())
        );
        assert_eq!(
            (
                created_review_state
                    .visual_source
                    .effective_render_source
                    .width,
                created_review_state
                    .visual_source
                    .effective_render_source
                    .height
            ),
            (8, 8)
        );
        let (native_width, native_height, target_width, target_height, recipe_json): (
            i64,
            i64,
            i64,
            i64,
            String,
        ) = connection
            .query_row(
                "SELECT provider_native_width, provider_native_height,
                        target_canvas_width, target_canvas_height,
                        normalization_recipe_json
                 FROM icon_ai_versions
                 WHERE icon_id = ?1
                   AND candidate_id = ?2
                   AND apply_kind = 'new_icon_root'",
                rusqlite::params![created.created_icon.id, candidate_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .unwrap();
        assert_eq!((native_width, native_height), (5, 13));
        assert_eq!((target_width, target_height), (8, 8));
        let recipe: serde_json::Value = serde_json::from_str(&recipe_json).unwrap();
        assert_eq!(recipe["kind"], "cover_crop");
        assert_eq!(fs::read(raw_path).unwrap(), candidate_bytes);
        assert_eq!(
            created_review_state
                .candidates
                .iter()
                .find(|candidate| candidate.id == candidate_id)
                .unwrap()
                .source
                .sha256,
            raw_sha
        );
    }
    fs::remove_dir_all(paths.root).unwrap();
}

#[test]
fn normalization_preview_reports_output_size_limits_for_both_commit_paths() {
    let paths = temp_paths();
    {
        let mut connection = open_database(&paths.database_path).unwrap();
        let collection =
            create_collection(&mut connection, Some("AI size preview".to_string())).unwrap();
        let icon = import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![ImportImageFilePayload {
                original_filename: "original.png".to_string(),
                bytes: png_bytes([20, 40, 220, 255]),
            }],
        )
        .unwrap()
        .imported_icons
        .into_iter()
        .next()
        .unwrap();
        connection
            .execute(
                "UPDATE collections SET max_bytes = 1 WHERE id = ?1",
                [&collection.id],
            )
            .unwrap();
        let review = import_local_ai_candidate(
            &mut connection,
            &paths,
            &collection.id,
            ImportAiCandidatePayload {
                icon_id: icon.id.clone(),
                service_surface: "gemini_web".to_string(),
                file: ImportImageFilePayload {
                    original_filename: "candidate.png".to_string(),
                    bytes: png_bytes([230, 80, 30, 255]),
                },
            },
        )
        .unwrap();
        let candidate_id = review.candidates[0].id.clone();
        let normalization = AiNormalizationOptionsPayload::default();
        let preview = preview_ai_candidate_normalization(
            &connection,
            &paths,
            &collection.id,
            PreviewAiCandidateNormalizationPayload {
                icon_id: icon.id.clone(),
                candidate_id: candidate_id.clone(),
                expected_revision: review.visual_source.activation_revision,
                normalization: normalization.clone(),
            },
        )
        .unwrap();

        assert!(!preview.current_icon_compatibility.allowed);
        assert_eq!(
            preview.current_icon_compatibility.reason_code.as_deref(),
            Some("ai_current_icon_output_too_large")
        );
        assert!(!preview.new_icon_compatibility.allowed);
        assert_eq!(
            preview.new_icon_compatibility.reason_code.as_deref(),
            Some("ai_new_icon_output_too_large")
        );

        let current_error = activate_ai_candidate(
            &mut connection,
            &paths,
            &collection.id,
            ActivateAiCandidatePayload {
                icon_id: icon.id.clone(),
                candidate_id: candidate_id.clone(),
                expected_revision: review.visual_source.activation_revision,
                normalization: normalization.clone(),
                expected_preview_signature: Some(preview.preview_signature.clone()),
            },
        )
        .unwrap_err();
        assert_eq!(current_error.code, "validation");

        let new_icon_error = create_ai_icon_root(
            &mut connection,
            &paths,
            &collection.id,
            CreateAiIconRootPayload {
                icon_id: icon.id,
                candidate_id,
                expected_revision: review.visual_source.activation_revision,
                normalization,
                expected_preview_signature: Some(preview.preview_signature),
            },
        )
        .unwrap_err();
        assert_eq!(new_icon_error.code, "validation");
    }
    fs::remove_dir_all(paths.root).unwrap();
}

fn managed_file_count(root: &std::path::Path) -> usize {
    if !root.is_dir() {
        return 0;
    }
    fs::read_dir(root)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .map(|path| {
            if path.is_dir() {
                managed_file_count(&path)
            } else {
                1
            }
        })
        .sum()
}

#[test]
fn failed_non_identity_apply_removes_new_source_artifacts_and_database_row() {
    let paths = temp_paths();
    {
        let mut connection = open_database(&paths.database_path).unwrap();
        let collection = create_collection(
            &mut connection,
            Some("AI 정규화 커밋 실패 신규 파일".to_string()),
        )
        .unwrap();
        let icon = import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![ImportImageFilePayload {
                original_filename: "original.png".to_string(),
                bytes: png_bytes([20, 40, 220, 255]),
            }],
        )
        .unwrap()
        .imported_icons
        .into_iter()
        .next()
        .unwrap();
        let review = import_local_ai_candidate(
            &mut connection,
            &paths,
            &collection.id,
            ImportAiCandidatePayload {
                icon_id: icon.id.clone(),
                service_surface: "gemini_web".to_string(),
                file: ImportImageFilePayload {
                    original_filename: "wide-candidate.png".to_string(),
                    bytes: png_bytes_with_dimensions(13, 5, [230, 80, 30, 180]),
                },
            },
        )
        .unwrap();
        let candidate_id = review.candidates[0].id.clone();
        let normalization = AiNormalizationOptionsPayload {
            mode: "contain_pad".to_string(),
            alignment: "center".to_string(),
            resize_filter: "nearest".to_string(),
            pad_rgba: [0, 0, 0, 0],
        };
        let preview = preview_ai_candidate_normalization(
            &connection,
            &paths,
            &collection.id,
            PreviewAiCandidateNormalizationPayload {
                icon_id: icon.id.clone(),
                candidate_id: candidate_id.clone(),
                expected_revision: review.visual_source.activation_revision,
                normalization: normalization.clone(),
            },
        )
        .unwrap();
        assert_eq!(preview.geometry.kind, "contain_pad");
        let normalized_bytes = fs::read(&preview.normalized_preview_path).unwrap();
        let normalized_sha = format!("{:x}", Sha256::digest(&normalized_bytes));
        let normalized_original_path = paths
            .originals_dir
            .join(&normalized_sha[..2])
            .join(format!("{normalized_sha}.png"));
        assert!(!normalized_original_path.exists());
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM source_files WHERE sha256 = ?1",
                    [&normalized_sha],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            0
        );

        let before = get_ai_review_state(&connection, &collection.id, &icon.id).unwrap();
        let source_count_before: i64 = connection
            .query_row("SELECT COUNT(*) FROM source_files", [], |row| row.get(0))
            .unwrap();
        let version_count_before: i64 = connection
            .query_row("SELECT COUNT(*) FROM icon_ai_versions", [], |row| {
                row.get(0)
            })
            .unwrap();
        let original_files_before = managed_file_count(&paths.originals_dir);
        let thumbnail_files_before = managed_file_count(&paths.source_file_thumbnails_dir);
        let staging_entries_before = managed_entry_count(&paths.ai_activation_staging_dir);
        let preview_files_before = managed_file_count(&paths.ai_activation_previews_dir);
        connection
            .execute_batch(
                "CREATE TEMP TRIGGER fail_ai_normalized_source_commit
                 BEFORE UPDATE OF current_preview_path ON icons
                 BEGIN
                   SELECT RAISE(ABORT, 'injected normalized source commit failure');
                 END;",
            )
            .unwrap();

        let error = activate_ai_candidate(
            &mut connection,
            &paths,
            &collection.id,
            ActivateAiCandidatePayload {
                icon_id: icon.id.clone(),
                candidate_id,
                expected_revision: review.visual_source.activation_revision,
                normalization,
                expected_preview_signature: Some(preview.preview_signature),
            },
        )
        .unwrap_err();
        assert!(!error.message.is_empty());

        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM source_files", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            source_count_before
        );
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM icon_ai_versions", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            version_count_before
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM source_files WHERE sha256 = ?1",
                    [&normalized_sha],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            0
        );
        assert!(!normalized_original_path.exists());
        assert_eq!(
            managed_file_count(&paths.originals_dir),
            original_files_before
        );
        assert_eq!(
            managed_file_count(&paths.source_file_thumbnails_dir),
            thumbnail_files_before
        );
        assert_eq!(
            managed_entry_count(&paths.ai_activation_staging_dir),
            staging_entries_before
        );
        assert_eq!(
            managed_file_count(&paths.ai_activation_previews_dir),
            preview_files_before
        );
        let after = get_ai_review_state(&connection, &collection.id, &icon.id).unwrap();
        assert_eq!(
            after.visual_source.activation_revision,
            before.visual_source.activation_revision
        );
        assert_eq!(after.visual_source.active_version_id, None);
        assert_eq!(
            after.visual_source.effective_render_source.id,
            before.visual_source.effective_render_source.id
        );
        assert_eq!(after.versions.len(), before.versions.len());
    }
    fs::remove_dir_all(paths.root).unwrap();
}

#[test]
fn failed_non_identity_apply_preserves_preexisting_deduped_source_artifacts() {
    let paths = temp_paths();
    {
        let mut connection = open_database(&paths.database_path).unwrap();
        let collection = create_collection(
            &mut connection,
            Some("AI 정규화 커밋 실패 dedupe".to_string()),
        )
        .unwrap();
        let icon = import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![ImportImageFilePayload {
                original_filename: "original.png".to_string(),
                bytes: png_bytes([20, 40, 220, 255]),
            }],
        )
        .unwrap()
        .imported_icons
        .into_iter()
        .next()
        .unwrap();
        let review = import_local_ai_candidate(
            &mut connection,
            &paths,
            &collection.id,
            ImportAiCandidatePayload {
                icon_id: icon.id.clone(),
                service_surface: "novelai_web".to_string(),
                file: ImportImageFilePayload {
                    original_filename: "wide-candidate.png".to_string(),
                    bytes: png_bytes_with_dimensions(13, 5, [30, 210, 100, 180]),
                },
            },
        )
        .unwrap();
        let candidate_id = review.candidates[0].id.clone();
        let normalization = AiNormalizationOptionsPayload {
            mode: "contain_pad".to_string(),
            alignment: "bottom_right".to_string(),
            resize_filter: "nearest".to_string(),
            pad_rgba: [0, 0, 0, 0],
        };
        let preview = preview_ai_candidate_normalization(
            &connection,
            &paths,
            &collection.id,
            PreviewAiCandidateNormalizationPayload {
                icon_id: icon.id.clone(),
                candidate_id: candidate_id.clone(),
                expected_revision: review.visual_source.activation_revision,
                normalization: normalization.clone(),
            },
        )
        .unwrap();
        let normalized_bytes = fs::read(&preview.normalized_preview_path).unwrap();
        let normalized_sha = format!("{:x}", Sha256::digest(&normalized_bytes));
        let seed_icon = import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![ImportImageFilePayload {
                original_filename: "normalized-dedupe-seed.png".to_string(),
                bytes: normalized_bytes,
            }],
        )
        .unwrap()
        .imported_icons
        .into_iter()
        .next()
        .unwrap();
        let (dedupe_source_id, dedupe_original_path): (String, String) = connection
            .query_row(
                "SELECT id, original_path_in_library
                 FROM source_files
                 WHERE sha256 = ?1",
                [&normalized_sha],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(seed_icon.source_file_id, dedupe_source_id);
        let dedupe_thumbnail_path = paths
            .source_file_thumbnails_dir
            .join(format!("{dedupe_source_id}.png"));
        let dedupe_original_bytes = fs::read(&dedupe_original_path).unwrap();
        let dedupe_thumbnail_bytes = fs::read(&dedupe_thumbnail_path).unwrap();
        let source_count_before: i64 = connection
            .query_row("SELECT COUNT(*) FROM source_files", [], |row| row.get(0))
            .unwrap();
        let version_count_before: i64 = connection
            .query_row("SELECT COUNT(*) FROM icon_ai_versions", [], |row| {
                row.get(0)
            })
            .unwrap();
        let original_files_before = managed_file_count(&paths.originals_dir);
        let thumbnail_files_before = managed_file_count(&paths.source_file_thumbnails_dir);
        let before = get_ai_review_state(&connection, &collection.id, &icon.id).unwrap();
        connection
            .execute_batch(
                "CREATE TEMP TRIGGER fail_ai_deduped_source_commit
                 BEFORE UPDATE OF current_preview_path ON icons
                 BEGIN
                   SELECT RAISE(ABORT, 'injected deduped source commit failure');
                 END;",
            )
            .unwrap();

        let error = activate_ai_candidate(
            &mut connection,
            &paths,
            &collection.id,
            ActivateAiCandidatePayload {
                icon_id: icon.id.clone(),
                candidate_id,
                expected_revision: review.visual_source.activation_revision,
                normalization,
                expected_preview_signature: Some(preview.preview_signature),
            },
        )
        .unwrap_err();
        assert!(!error.message.is_empty());

        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM source_files", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            source_count_before
        );
        assert_eq!(
            connection
                .query_row("SELECT COUNT(*) FROM icon_ai_versions", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            version_count_before
        );
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM source_files WHERE id = ?1 AND sha256 = ?2",
                    rusqlite::params![dedupe_source_id, normalized_sha],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            1
        );
        assert_eq!(
            fs::read(&dedupe_original_path).unwrap(),
            dedupe_original_bytes
        );
        assert_eq!(
            fs::read(&dedupe_thumbnail_path).unwrap(),
            dedupe_thumbnail_bytes
        );
        assert_eq!(
            managed_file_count(&paths.originals_dir),
            original_files_before
        );
        assert_eq!(
            managed_file_count(&paths.source_file_thumbnails_dir),
            thumbnail_files_before
        );
        let after = get_ai_review_state(&connection, &collection.id, &icon.id).unwrap();
        assert_eq!(
            after.visual_source.activation_revision,
            before.visual_source.activation_revision
        );
        assert_eq!(after.visual_source.active_version_id, None);
        assert_eq!(
            after.visual_source.effective_render_source.id,
            before.visual_source.effective_render_source.id
        );
        assert_eq!(after.versions.len(), before.versions.len());
    }
    fs::remove_dir_all(paths.root).unwrap();
}

#[test]
fn static_ai_candidate_is_rejected_for_animated_gif_target_without_mutation() {
    let paths = temp_paths();
    {
        let mut connection = open_database(&paths.database_path).unwrap();
        let collection =
            create_collection(&mut connection, Some("AI GIF 대상 호환성".to_string())).unwrap();
        let gif_source_bytes = animated_gif_bytes();
        let icon = import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![ImportImageFilePayload {
                original_filename: "animated-original.gif".to_string(),
                bytes: gif_source_bytes.clone(),
            }],
        )
        .unwrap()
        .imported_icons
        .into_iter()
        .next()
        .unwrap();
        let review = import_local_ai_candidate(
            &mut connection,
            &paths,
            &collection.id,
            ImportAiCandidatePayload {
                icon_id: icon.id.clone(),
                service_surface: "gemini_web".to_string(),
                file: ImportImageFilePayload {
                    original_filename: "static-candidate.png".to_string(),
                    bytes: png_bytes_with_dimensions(13, 5, [230, 80, 30, 255]),
                },
            },
        )
        .unwrap();
        assert!(review.visual_source.effective_render_source.is_animated);
        let candidate_id = review.candidates[0].id.clone();
        let normalization = AiNormalizationOptionsPayload::default();
        let preview = preview_ai_candidate_normalization(
            &connection,
            &paths,
            &collection.id,
            PreviewAiCandidateNormalizationPayload {
                icon_id: icon.id.clone(),
                candidate_id: candidate_id.clone(),
                expected_revision: review.visual_source.activation_revision,
                normalization: normalization.clone(),
            },
        )
        .unwrap();
        assert!(!preview.current_icon_compatibility.allowed);
        assert_eq!(
            preview.current_icon_compatibility.reason_code.as_deref(),
            Some("ai_normalization_animation_target")
        );

        let before = get_ai_review_state(&connection, &collection.id, &icon.id).unwrap();
        let gif_path = before
            .visual_source
            .effective_render_source
            .original_image_url
            .clone();
        let gif_bytes_before = fs::read(&gif_path).unwrap();
        assert_eq!(gif_bytes_before, gif_source_bytes);
        let icon_preview_before: Option<String> = connection
            .query_row(
                "SELECT current_preview_path FROM icons WHERE id = ?1",
                [&icon.id],
                |row| row.get(0),
            )
            .unwrap();
        let database_counts_before: (i64, i64, i64, i64) = connection
            .query_row(
                "SELECT
                   (SELECT COUNT(*) FROM source_files),
                   (SELECT COUNT(*) FROM ai_requests),
                   (SELECT COUNT(*) FROM ai_candidates),
                   (SELECT COUNT(*) FROM icon_ai_versions)",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        let original_files_before = managed_file_count(&paths.originals_dir);
        let thumbnail_files_before = managed_file_count(&paths.source_file_thumbnails_dir);
        let staging_entries_before = managed_entry_count(&paths.ai_activation_staging_dir);
        let activation_preview_entries_before =
            managed_entry_count(&paths.ai_activation_previews_dir);

        let error = activate_ai_candidate(
            &mut connection,
            &paths,
            &collection.id,
            ActivateAiCandidatePayload {
                icon_id: icon.id.clone(),
                candidate_id,
                expected_revision: review.visual_source.activation_revision,
                normalization,
                expected_preview_signature: Some(preview.preview_signature),
            },
        )
        .unwrap_err();
        assert_eq!(error.code, "ai_normalization_animation_target");

        let after = get_ai_review_state(&connection, &collection.id, &icon.id).unwrap();
        assert_eq!(after.visual_source.active_version_id, None);
        assert_eq!(
            after.visual_source.activation_revision,
            before.visual_source.activation_revision
        );
        assert_eq!(
            after.visual_source.effective_render_source.id,
            before.visual_source.effective_render_source.id
        );
        assert!(after.visual_source.effective_render_source.is_animated);
        assert_eq!(after.versions.len(), before.versions.len());
        assert_eq!(fs::read(&gif_path).unwrap(), gif_bytes_before);
        let icon_preview_after: Option<String> = connection
            .query_row(
                "SELECT current_preview_path FROM icons WHERE id = ?1",
                [&icon.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(icon_preview_after, icon_preview_before);
        let database_counts_after: (i64, i64, i64, i64) = connection
            .query_row(
                "SELECT
                   (SELECT COUNT(*) FROM source_files),
                   (SELECT COUNT(*) FROM ai_requests),
                   (SELECT COUNT(*) FROM ai_candidates),
                   (SELECT COUNT(*) FROM icon_ai_versions)",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(database_counts_after, database_counts_before);
        assert_eq!(
            managed_file_count(&paths.originals_dir),
            original_files_before
        );
        assert_eq!(
            managed_file_count(&paths.source_file_thumbnails_dir),
            thumbnail_files_before
        );
        assert_eq!(
            managed_entry_count(&paths.ai_activation_staging_dir),
            staging_entries_before
        );
        assert_eq!(
            managed_entry_count(&paths.ai_activation_previews_dir),
            activation_preview_entries_before
        );
    }
    fs::remove_dir_all(paths.root).unwrap();
}

#[test]
fn local_candidate_insert_failure_removes_prepared_source_artifacts_and_rows() {
    let paths = temp_paths();
    {
        let mut connection = open_database(&paths.database_path).unwrap();
        let collection = create_collection(
            &mut connection,
            Some("AI 후보 import 커밋 실패".to_string()),
        )
        .unwrap();
        let icon = import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![ImportImageFilePayload {
                original_filename: "original.png".to_string(),
                bytes: png_bytes([20, 40, 220, 255]),
            }],
        )
        .unwrap()
        .imported_icons
        .into_iter()
        .next()
        .unwrap();
        let candidate_bytes = png_bytes_with_dimensions(13, 5, [170, 30, 210, 180]);
        let candidate_sha = format!("{:x}", Sha256::digest(&candidate_bytes));
        let candidate_original_path = paths
            .originals_dir
            .join(&candidate_sha[..2])
            .join(format!("{candidate_sha}.png"));
        assert!(!candidate_original_path.exists());
        let before = get_ai_review_state(&connection, &collection.id, &icon.id).unwrap();
        let database_counts_before: (i64, i64, i64) = connection
            .query_row(
                "SELECT
                   (SELECT COUNT(*) FROM source_files),
                   (SELECT COUNT(*) FROM ai_requests),
                   (SELECT COUNT(*) FROM ai_candidates)",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        let original_files_before = managed_file_count(&paths.originals_dir);
        let thumbnail_files_before = managed_file_count(&paths.source_file_thumbnails_dir);
        connection
            .execute_batch(
                "CREATE TEMP TRIGGER fail_local_ai_candidate_insert
                 BEFORE INSERT ON ai_candidates
                 BEGIN
                   SELECT RAISE(ABORT, 'injected local candidate insert failure');
                 END;",
            )
            .unwrap();

        let error = import_local_ai_candidate(
            &mut connection,
            &paths,
            &collection.id,
            ImportAiCandidatePayload {
                icon_id: icon.id.clone(),
                service_surface: "gemini_web".to_string(),
                file: ImportImageFilePayload {
                    original_filename: "candidate-insert-failure.png".to_string(),
                    bytes: candidate_bytes,
                },
            },
        )
        .unwrap_err();
        assert!(!error.message.is_empty());

        let database_counts_after: (i64, i64, i64) = connection
            .query_row(
                "SELECT
                   (SELECT COUNT(*) FROM source_files),
                   (SELECT COUNT(*) FROM ai_requests),
                   (SELECT COUNT(*) FROM ai_candidates)",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(database_counts_after, database_counts_before);
        assert_eq!(
            connection
                .query_row(
                    "SELECT COUNT(*) FROM source_files WHERE sha256 = ?1",
                    [&candidate_sha],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            0
        );
        assert!(!candidate_original_path.exists());
        assert_eq!(
            managed_file_count(&paths.originals_dir),
            original_files_before
        );
        assert_eq!(
            managed_file_count(&paths.source_file_thumbnails_dir),
            thumbnail_files_before
        );
        let after = get_ai_review_state(&connection, &collection.id, &icon.id).unwrap();
        assert_eq!(after.candidates.len(), before.candidates.len());
        assert_eq!(after.versions.len(), before.versions.len());
        assert_eq!(
            after.visual_source.activation_revision,
            before.visual_source.activation_revision
        );
        assert_eq!(
            after.visual_source.effective_render_source.id,
            before.visual_source.effective_render_source.id
        );
    }
    fs::remove_dir_all(paths.root).unwrap();
}
#[test]
fn unavailable_inactive_candidate_and_version_remain_visible_and_mutations_fail_closed() {
    let paths = temp_paths();
    {
        let mut connection = open_database(&paths.database_path).unwrap();
        let collection =
            create_collection(&mut connection, Some("AI 손상 이력 표시".to_string())).unwrap();
        let icon = import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![ImportImageFilePayload {
                original_filename: "original.png".to_string(),
                bytes: png_bytes([20, 40, 220, 255]),
            }],
        )
        .unwrap()
        .imported_icons
        .into_iter()
        .next()
        .unwrap();
        let imported = import_local_ai_candidate(
            &mut connection,
            &paths,
            &collection.id,
            ImportAiCandidatePayload {
                icon_id: icon.id.clone(),
                service_surface: "gemini_web".to_string(),
                file: ImportImageFilePayload {
                    original_filename: "wide-candidate.png".to_string(),
                    bytes: png_bytes_with_dimensions(13, 5, [230, 80, 30, 180]),
                },
            },
        )
        .unwrap();
        let candidate = imported.candidates.first().unwrap();
        assert!(candidate.is_available);
        assert_eq!(candidate.unavailable_reason, None);
        let candidate_id = candidate.id.clone();
        let candidate_source_id = candidate.source.id.clone();
        let candidate_source_path = candidate.source.original_image_url.clone();
        let normalization = AiNormalizationOptionsPayload::default();
        let preview = preview_ai_candidate_normalization(
            &connection,
            &paths,
            &collection.id,
            PreviewAiCandidateNormalizationPayload {
                icon_id: icon.id.clone(),
                candidate_id: candidate_id.clone(),
                expected_revision: imported.visual_source.activation_revision,
                normalization: normalization.clone(),
            },
        )
        .unwrap();
        let applied = activate_ai_candidate(
            &mut connection,
            &paths,
            &collection.id,
            ActivateAiCandidatePayload {
                icon_id: icon.id.clone(),
                candidate_id: candidate_id.clone(),
                expected_revision: imported.visual_source.activation_revision,
                normalization: normalization.clone(),
                expected_preview_signature: Some(preview.preview_signature.clone()),
            },
        )
        .unwrap();
        let version = applied
            .review_state
            .versions
            .iter()
            .find(|version| version.candidate_id == candidate_id)
            .unwrap();
        assert!(version.is_available);
        assert_eq!(version.unavailable_reason, None);
        let version_id = version.id.clone();
        let version_source_id = version.source.id.clone();
        let version_source_path = version.source.original_image_url.clone();
        assert_ne!(candidate_source_id, version_source_id);
        assert_ne!(candidate_source_path, version_source_path);

        let restored = restore_ai_version(
            &mut connection,
            &paths,
            &collection.id,
            RestoreAiVersionPayload {
                icon_id: icon.id.clone(),
                version_id: None,
                expected_revision: applied.review_state.visual_source.activation_revision,
            },
        )
        .unwrap();
        let stable_revision = restored.review_state.visual_source.activation_revision;
        let stable_source_id = restored
            .review_state
            .visual_source
            .effective_render_source
            .id
            .clone();
        assert_eq!(restored.review_state.visual_source.active_version_id, None);

        fs::remove_file(&candidate_source_path).unwrap();
        fs::remove_file(&version_source_path).unwrap();

        let unavailable = get_ai_review_state(&connection, &collection.id, &icon.id).unwrap();
        let unavailable_candidate = unavailable
            .candidates
            .iter()
            .find(|candidate| candidate.id == candidate_id)
            .unwrap();
        assert!(!unavailable_candidate.is_available);
        assert!(unavailable_candidate
            .unavailable_reason
            .as_deref()
            .is_some_and(|reason| reason.contains("찾을 수 없어")));
        assert_eq!(unavailable_candidate.source.id, candidate_source_id);
        let unavailable_version = unavailable
            .versions
            .iter()
            .find(|version| version.id == version_id)
            .unwrap();
        assert!(!unavailable_version.is_available);
        assert!(unavailable_version
            .unavailable_reason
            .as_deref()
            .is_some_and(|reason| reason.contains("찾을 수 없어")));
        assert_eq!(unavailable_version.source.id, version_source_id);

        let apply_error = activate_ai_candidate(
            &mut connection,
            &paths,
            &collection.id,
            ActivateAiCandidatePayload {
                icon_id: icon.id.clone(),
                candidate_id: candidate_id.clone(),
                expected_revision: stable_revision,
                normalization,
                expected_preview_signature: Some(preview.preview_signature),
            },
        )
        .unwrap_err();
        assert_eq!(apply_error.code, "ai_source_repair_required");
        let after_apply = get_ai_review_state(&connection, &collection.id, &icon.id).unwrap();
        assert_eq!(
            after_apply.visual_source.activation_revision,
            stable_revision
        );
        assert_eq!(after_apply.visual_source.active_version_id, None);
        assert_eq!(
            after_apply.visual_source.effective_render_source.id,
            stable_source_id
        );

        let restore_error = restore_ai_version(
            &mut connection,
            &paths,
            &collection.id,
            RestoreAiVersionPayload {
                icon_id: icon.id.clone(),
                version_id: Some(version_id.clone()),
                expected_revision: stable_revision,
            },
        )
        .unwrap_err();
        assert_eq!(restore_error.code, "ai_source_repair_required");
        let after_restore = get_ai_review_state(&connection, &collection.id, &icon.id).unwrap();
        assert_eq!(
            after_restore.visual_source.activation_revision,
            stable_revision
        );
        assert_eq!(after_restore.visual_source.active_version_id, None);
        assert_eq!(
            after_restore.visual_source.effective_render_source.id,
            stable_source_id
        );
        assert!(after_restore
            .candidates
            .iter()
            .any(|candidate| candidate.id == candidate_id && !candidate.is_available));
        assert!(after_restore
            .versions
            .iter()
            .any(|version| version.id == version_id && !version.is_available));
    }
    fs::remove_dir_all(paths.root).unwrap();
}
