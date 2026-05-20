pub mod analyzer;
pub mod cache;
pub mod candidate;
pub mod gif_optimizer;
pub mod jobs;
pub mod settings;
pub mod static_optimizer;
pub mod variants;

use rusqlite::Connection;

use crate::db::repositories::optimization as optimization_repository;
use crate::error::{AppError, AppResult};
use crate::models::{
    ActiveVariantDto, ApplyOptimizationResultDto, ClearOptimizationResultDto,
    ExportAssetAnalysisDto, GifPlaybackPreviewResultDto, OptimizationAdvancedSettingsPayload,
    OptimizationCandidateDto, OptimizationResultDto,
};
use crate::paths::AppPaths;

pub fn analyze_export_asset_candidate(
    connection: &Connection,
    paths: &AppPaths,
    icon_id: &str,
    profile_id: &str,
    piece_id: Option<&str>,
) -> AppResult<ExportAssetAnalysisDto> {
    let baseline = analyzer::render_baseline(connection, paths, icon_id, profile_id, piece_id)?;
    Ok(baseline.analysis)
}

pub fn generate_gif_optimization_candidates(
    connection: &Connection,
    paths: &AppPaths,
    icon_id: &str,
    profile_id: &str,
    piece_id: Option<&str>,
    _mode: Option<String>,
    advanced_settings: Option<OptimizationAdvancedSettingsPayload>,
) -> AppResult<OptimizationResultDto> {
    let baseline = analyzer::render_baseline(connection, paths, icon_id, profile_id, piece_id)?;
    if advanced_settings.is_none()
        && baseline.analysis.baseline_bytes <= baseline.analysis.target_max_bytes
    {
        return Ok(OptimizationResultDto {
            analysis: baseline.analysis,
            candidates: Vec::new(),
            already_passes: true,
            fallback_suggestions: Vec::new(),
            message: "현재 GIF는 제한을 통과합니다. 추가 최적화가 필요 없습니다.".to_string(),
        });
    }

    let candidates = gif_optimizer::generate_candidates(
        connection,
        paths,
        &baseline,
        advanced_settings.as_ref(),
    )?;
    Ok(OptimizationResultDto {
        analysis: baseline.analysis,
        candidates,
        already_passes: false,
        fallback_suggestions: fallback_suggestions("gif"),
        message:
            "최적화 후보가 생성되었습니다. 원본 파일은 보존되며, 선택한 후보만 export에 사용됩니다."
                .to_string(),
    })
}

pub fn generate_static_optimization_candidates(
    connection: &Connection,
    paths: &AppPaths,
    icon_id: &str,
    profile_id: &str,
    piece_id: Option<&str>,
    _mode: Option<String>,
    advanced_settings: Option<OptimizationAdvancedSettingsPayload>,
) -> AppResult<OptimizationResultDto> {
    let baseline = analyzer::render_baseline(connection, paths, icon_id, profile_id, piece_id)?;
    if advanced_settings.is_none()
        && baseline.analysis.baseline_bytes <= baseline.analysis.target_max_bytes
    {
        return Ok(OptimizationResultDto {
            analysis: baseline.analysis,
            candidates: Vec::new(),
            already_passes: true,
            fallback_suggestions: Vec::new(),
            message: "현재 파일은 제한을 통과합니다. 추가 최적화가 필요 없습니다.".to_string(),
        });
    }

    let format = baseline.analysis.format.clone();
    let candidates = static_optimizer::generate_candidates(
        connection,
        paths,
        &baseline,
        advanced_settings.as_ref(),
    )?;
    Ok(OptimizationResultDto {
        analysis: baseline.analysis,
        candidates,
        already_passes: false,
        fallback_suggestions: fallback_suggestions(&format),
        message:
            "최적화 후보가 생성되었습니다. 원본 파일은 보존되며, 선택한 후보만 export에 사용됩니다."
                .to_string(),
    })
}

pub fn list_optimization_candidates(
    connection: &Connection,
    icon_id: &str,
    profile_id: &str,
    piece_id: Option<&str>,
) -> AppResult<Vec<OptimizationCandidateDto>> {
    let target = analyzer::load_target(connection, icon_id, profile_id, piece_id)?;
    let candidates = optimization_repository::list_candidates(
        connection,
        icon_id,
        profile_id,
        Some(&target.piece_id),
    )?;
    Ok(candidates
        .iter()
        .map(|candidate| {
            optimization_repository::to_candidate_dto(
                candidate,
                target.profile.max_bytes,
                None,
                None,
            )
        })
        .collect())
}

pub fn apply_optimization_candidate(
    connection: &Connection,
    candidate_id: &str,
) -> AppResult<ApplyOptimizationResultDto> {
    let variant = optimization_repository::set_active_variant(connection, candidate_id)?;
    let target_max_bytes = variant
        .profile_id
        .as_deref()
        .and_then(|profile_id| profile_max_bytes(connection, profile_id).ok())
        .unwrap_or(variant.byte_size);
    let candidate =
        optimization_repository::to_candidate_dto(&variant, target_max_bytes, None, None);
    Ok(ApplyOptimizationResultDto {
        message: format!(
            "{} 후보를 적용했습니다. Export 검증을 다시 실행합니다.",
            candidate.preset
        ),
        candidate,
    })
}

pub fn apply_optimization_candidate_to_preview(
    connection: &Connection,
    candidate_id: &str,
) -> AppResult<ApplyOptimizationResultDto> {
    let variant = optimization_repository::set_active_variant(connection, candidate_id)?;
    let variant = optimization_repository::promote_variant_to_preview(connection, &variant.id)?;
    let target_max_bytes = variant
        .profile_id
        .as_deref()
        .and_then(|profile_id| profile_max_bytes(connection, profile_id).ok())
        .unwrap_or(variant.byte_size);
    let candidate =
        optimization_repository::to_candidate_dto(&variant, target_max_bytes, None, None);
    Ok(ApplyOptimizationResultDto {
        message: "GIF 재생 FPS를 실제 미리보기와 내보내기 결과에 적용했습니다.".to_string(),
        candidate,
    })
}

pub fn preview_gif_playback_fps(
    connection: &Connection,
    paths: &AppPaths,
    icon_id: &str,
    playback_fps: i64,
) -> AppResult<GifPlaybackPreviewResultDto> {
    let (source_path, is_animated): (String, i64) = connection
        .query_row(
            "SELECT s.original_path_in_library, s.is_animated
             FROM source_files s
             JOIN icons i ON i.source_file_id = s.id
             WHERE i.id = ?1
               AND i.deleted_at IS NULL",
            [icon_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(AppError::from)?;

    if is_animated == 0 || !source_path.to_ascii_lowercase().ends_with(".gif") {
        return Err(AppError::new(
            "validation",
            "GIF 아이콘에서만 재생 FPS 미리보기를 만들 수 있습니다.",
        ));
    }

    let playback_fps = playback_fps.clamp(1, 60);
    let preview_dir = paths
        .temp_export_dir
        .join("gif-playback-preview")
        .join(icon_id);
    std::fs::create_dir_all(&preview_dir)?;
    let final_path = preview_dir.join(format!("fps_{playback_fps}.gif"));
    let temp_path = final_path.with_extension("tmp");
    gif_optimizer::write_gif_playback_fps_candidate_streaming(
        &temp_path,
        std::path::Path::new(&source_path),
        10,
        Some(playback_fps),
    )?;
    analyzer::move_temp_file(&temp_path, &final_path)?;

    Ok(GifPlaybackPreviewResultDto {
        preview_path: final_path.to_string_lossy().to_string(),
        playback_fps,
        generated_at: chrono_like_timestamp(),
    })
}

pub fn apply_gif_original_playback_to_preview(
    connection: &Connection,
    paths: &AppPaths,
    icon_id: &str,
    profile_id: &str,
    piece_id: Option<&str>,
) -> AppResult<ApplyOptimizationResultDto> {
    let baseline = analyzer::render_baseline(connection, paths, icon_id, profile_id, piece_id)?;
    let candidate_id = baseline.analysis.baseline_variant_id.clone();
    let variant = optimization_repository::set_active_variant(connection, &candidate_id)?;
    let variant = optimization_repository::promote_variant_to_preview(connection, &variant.id)?;
    let candidate = optimization_repository::to_candidate_dto(
        &variant,
        baseline.target.profile.max_bytes,
        baseline.frame_count,
        baseline.duration_ms,
    );

    Ok(ApplyOptimizationResultDto {
        message: "원본 GIF 프레임 지연 시간을 실제 미리보기와 내보내기 결과에 적용했습니다."
            .to_string(),
        candidate,
    })
}

fn profile_max_bytes(connection: &Connection, profile_id: &str) -> AppResult<i64> {
    Ok(connection.query_row(
        "SELECT max_bytes FROM export_profiles WHERE id = ?1",
        [profile_id],
        |row| row.get(0),
    )?)
}

fn chrono_like_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    millis.to_string()
}

pub fn clear_optimization_candidate(
    connection: &Connection,
    icon_id: &str,
    profile_id: &str,
    piece_id: Option<&str>,
) -> AppResult<ClearOptimizationResultDto> {
    let cleared_count =
        optimization_repository::clear_active_variant(connection, icon_id, profile_id, piece_id)?;
    Ok(ClearOptimizationResultDto {
        icon_id: icon_id.to_string(),
        profile_id: profile_id.to_string(),
        piece_id: piece_id.map(str::to_string),
        cleared_count,
        message: "원본 export 결과를 사용하도록 되돌렸습니다.".to_string(),
    })
}

pub fn get_active_export_variant(
    connection: &Connection,
    icon_id: &str,
    profile_id: &str,
    piece_id: Option<&str>,
) -> AppResult<Option<ActiveVariantDto>> {
    let target = analyzer::load_target(connection, icon_id, profile_id, piece_id)?;
    let active = optimization_repository::find_active_variant(
        connection,
        &target.icon_id,
        &target.profile.id,
        &target.piece_id,
        &target.source_hash,
        &target.crop_hash,
        &target.profile_hash,
        &target.output_format,
    )?;
    Ok(active.map(|variant| ActiveVariantDto {
        candidate: optimization_repository::to_candidate_dto(
            &variant,
            target.profile.max_bytes,
            None,
            None,
        ),
        stale: false,
    }))
}

pub fn revalidate_export_item(
    connection: &Connection,
    paths: &AppPaths,
    icon_id: &str,
    profile_id: &str,
    piece_id: Option<&str>,
) -> AppResult<ExportAssetAnalysisDto> {
    analyze_export_asset_candidate(connection, paths, icon_id, profile_id, piece_id)
}

fn fallback_suggestions(format: &str) -> Vec<String> {
    match format {
        "gif" => vec![
            "더 강하게 압축".to_string(),
            "FPS 줄이기".to_string(),
            "정지 PNG로 변환".to_string(),
            "crop 영역 조정".to_string(),
            "export에서 제외".to_string(),
            "export는 하되 업로드 불가로 표시".to_string(),
        ],
        "jpg" => vec![
            "더 낮은 JPG 품질 사용".to_string(),
            "crop 영역 조정".to_string(),
            "프로필 용량 제한 조정".to_string(),
            "export는 하되 업로드 불가로 표시".to_string(),
        ],
        _ => vec![
            "crop 영역 조정".to_string(),
            "투명도가 필요 없다면 JPG 프로필 사용 검토".to_string(),
            "export는 하되 업로드 불가로 표시".to_string(),
        ],
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    use image::{DynamicImage, ImageBuffer, ImageFormat, Rgba};
    use rusqlite::Connection;

    use super::{
        apply_gif_original_playback_to_preview, apply_optimization_candidate,
        apply_optimization_candidate_to_preview, generate_gif_optimization_candidates,
        generate_static_optimization_candidates, get_active_export_variant,
        preview_gif_playback_fps,
    };
    use crate::db::migrations;
    use crate::db::repositories::collections::create_collection;
    use crate::db::repositories::export_profiles::{
        list_export_profiles, update_export_profile_settings,
    };
    use crate::db::repositories::imports::import_image_files;
    use crate::export::export_collection;
    use crate::models::{
        ExportRequestPayload, ImportImageFilePayload, OptimizationAdvancedSettingsPayload,
    };
    use crate::paths::AppPaths;

    #[test]
    fn static_jpg_candidate_can_be_applied_and_used_by_export() {
        let mut connection = connection();
        let paths = temp_paths("pmtconcon-static-optimization");
        let collection =
            create_collection(&mut connection, Some("static optimization".to_string())).unwrap();
        let imported = import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![ImportImageFilePayload {
                original_filename: "source.png".to_string(),
                bytes: png_bytes(220, 220),
            }],
        )
        .unwrap();
        let icon_id = imported.imported_icons[0].id.clone();
        let piece_id = imported.imported_icons[0].pieces[0].id.clone();
        let profile_id = custom_profile_id(&connection, &collection.id);
        let payload = payload(&profile_id, "jpg", 100);
        update_export_profile_settings(&connection, &collection.id, &payload).unwrap();

        let result = generate_static_optimization_candidates(
            &connection,
            &paths,
            &icon_id,
            &profile_id,
            Some(&piece_id),
            None,
            None,
        )
        .unwrap();

        assert!(!result.candidates.is_empty());
        assert!(result
            .candidates
            .iter()
            .all(|candidate| Path::new(&candidate.path).is_file()));

        let smallest = result
            .candidates
            .iter()
            .find(|candidate| candidate.preset == "smallest")
            .unwrap();
        apply_optimization_candidate(&connection, &smallest.id).unwrap();
        assert!(
            get_active_export_variant(&connection, &icon_id, &profile_id, Some(&piece_id),)
                .unwrap()
                .is_some()
        );

        let export = export_collection(&mut connection, &paths, &collection.id, &payload).unwrap();
        let report = std::fs::read_to_string(export.report_txt_path.as_ref().unwrap()).unwrap();
        assert!(report.contains("optimized_variant"));

        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn gif_candidates_are_actual_measured_files_and_original_is_preserved() {
        let mut connection = connection();
        let paths = temp_paths("pmtconcon-gif-optimization");
        let collection =
            create_collection(&mut connection, Some("gif optimization".to_string())).unwrap();
        let source_bytes = animated_gif_bytes();
        let imported = import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![ImportImageFilePayload {
                original_filename: "source.gif".to_string(),
                bytes: source_bytes.clone(),
            }],
        )
        .unwrap();
        let icon_id = imported.imported_icons[0].id.clone();
        let piece_id = imported.imported_icons[0].pieces[0].id.clone();
        let original_path: String = connection
            .query_row(
                "SELECT s.original_path_in_library
                 FROM source_files s
                 JOIN icons i ON i.source_file_id = s.id
                 WHERE i.id = ?1",
                [&icon_id],
                |row| row.get(0),
            )
            .unwrap();
        let profile_id = custom_profile_id(&connection, &collection.id);
        let payload = payload(&profile_id, "gif", 100);
        update_export_profile_settings(&connection, &collection.id, &payload).unwrap();

        let result = generate_gif_optimization_candidates(
            &connection,
            &paths,
            &icon_id,
            &profile_id,
            Some(&piece_id),
            None,
            None,
        )
        .unwrap();

        assert!(!result.candidates.is_empty());
        for candidate in &result.candidates {
            assert!(candidate.measured_byte_size > 0);
            assert!(Path::new(&candidate.path).is_file());
            assert_eq!(candidate.format, "gif");
        }
        assert_eq!(std::fs::read(original_path).unwrap(), source_bytes);

        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn gif_advanced_playback_fps_generates_candidate_even_when_baseline_passes() {
        let mut connection = connection();
        let paths = temp_paths("pmtconcon-gif-playback-fps");
        let collection =
            create_collection(&mut connection, Some("gif playback fps".to_string())).unwrap();
        let imported = import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![ImportImageFilePayload {
                original_filename: "source.gif".to_string(),
                bytes: animated_gif_bytes(),
            }],
        )
        .unwrap();
        let icon_id = imported.imported_icons[0].id.clone();
        let piece_id = imported.imported_icons[0].pieces[0].id.clone();
        let profile_id = custom_profile_id(&connection, &collection.id);
        let payload = payload(&profile_id, "gif", 10_000_000);
        update_export_profile_settings(&connection, &collection.id, &payload).unwrap();

        let preview = preview_gif_playback_fps(&connection, &paths, &icon_id, 12).unwrap();
        assert_eq!(preview.playback_fps, 12);
        assert!(Path::new(&preview.preview_path).is_file());

        let result = generate_gif_optimization_candidates(
            &connection,
            &paths,
            &icon_id,
            &profile_id,
            Some(&piece_id),
            Some("custom".to_string()),
            Some(OptimizationAdvancedSettingsPayload {
                target_max_bytes: None,
                safety_margin_percent: None,
                fps_limit: None,
                playback_fps: Some(10),
                frame_step: None,
                color_limit: None,
                jpeg_quality: None,
            }),
        )
        .unwrap();

        assert!(!result.already_passes);
        assert_eq!(result.candidates.len(), 1);
        let quality = result
            .candidates
            .iter()
            .find(|candidate| candidate.preset == "quality")
            .unwrap();
        assert_eq!(quality.frame_count, Some(8));
        assert!(quality.duration_ms.unwrap_or_default() >= 780);

        let applied = apply_optimization_candidate_to_preview(&connection, &quality.id).unwrap();
        assert_eq!(applied.candidate.id, quality.id);
        let (current_preview_path, generated_preview_path): (Option<String>, Option<String>) =
            connection
                .query_row(
                    "SELECT i.current_preview_path, p.generated_preview_path
                     FROM icons i
                     JOIN icon_pieces p ON p.icon_id = i.id
                     WHERE i.id = ?1
                       AND p.id = ?2",
                    [&icon_id, &piece_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap();
        assert_eq!(current_preview_path.as_deref(), Some(quality.path.as_str()));
        assert_eq!(
            generated_preview_path.as_deref(),
            Some(quality.path.as_str())
        );

        let restored = apply_gif_original_playback_to_preview(
            &connection,
            &paths,
            &icon_id,
            &profile_id,
            Some(&piece_id),
        )
        .unwrap();
        assert_ne!(restored.candidate.path, quality.path);
        let restored_preview_path: Option<String> = connection
            .query_row(
                "SELECT current_preview_path FROM icons WHERE id = ?1",
                [&icon_id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            restored_preview_path.as_deref(),
            Some(restored.candidate.path.as_str())
        );

        std::fs::remove_dir_all(paths.root).unwrap();
    }

    #[test]
    fn gif_playback_fps_handles_many_frames_without_frame_reduction() {
        let mut connection = connection();
        let paths = temp_paths("pmtconcon-gif-playback-fps-many");
        let collection =
            create_collection(&mut connection, Some("gif playback fps many".to_string())).unwrap();
        let imported = import_image_files(
            &mut connection,
            &paths,
            &collection.id,
            vec![ImportImageFilePayload {
                original_filename: "many.gif".to_string(),
                bytes: animated_gif_bytes_with_frames(80),
            }],
        )
        .unwrap();
        let icon_id = imported.imported_icons[0].id.clone();
        let piece_id = imported.imported_icons[0].pieces[0].id.clone();
        let profile_id = custom_profile_id(&connection, &collection.id);
        let payload = payload(&profile_id, "gif", 10_000_000);
        update_export_profile_settings(&connection, &collection.id, &payload).unwrap();

        let result = generate_gif_optimization_candidates(
            &connection,
            &paths,
            &icon_id,
            &profile_id,
            Some(&piece_id),
            Some("custom".to_string()),
            Some(OptimizationAdvancedSettingsPayload {
                target_max_bytes: None,
                safety_margin_percent: None,
                fps_limit: None,
                playback_fps: Some(20),
                frame_step: None,
                color_limit: None,
                jpeg_quality: None,
            }),
        )
        .unwrap();

        assert_eq!(result.candidates.len(), 1);
        let candidate = &result.candidates[0];
        assert_eq!(candidate.frame_count, Some(80));
        assert_eq!(candidate.duration_ms, Some(4_000));

        std::fs::remove_dir_all(paths.root).unwrap();
    }

    fn connection() -> Connection {
        let mut connection = Connection::open_in_memory().unwrap();
        migrations::run(&mut connection).unwrap();
        connection
    }

    fn temp_paths(prefix: &str) -> AppPaths {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        AppPaths::prepare(std::env::temp_dir().join(format!("{prefix}-{suffix}"))).unwrap()
    }

    fn custom_profile_id(connection: &Connection, collection_id: &str) -> String {
        list_export_profiles(connection, collection_id)
            .unwrap()
            .into_iter()
            .find(|profile| profile.profile_type == "custom")
            .unwrap()
            .id
    }

    fn payload(profile_id: &str, target_format: &str, max_bytes: i64) -> ExportRequestPayload {
        ExportRequestPayload {
            profile_id: profile_id.to_string(),
            target_format: target_format.to_string(),
            target_cell_width: 200,
            target_cell_height: 200,
            max_bytes,
            filename_mode: "sequence".to_string(),
            include_alt_txt: true,
            strict_warnings: false,
            output_directory: None,
            open_folder_after_export: false,
            open_alt_txt_after_export: false,
            excluded_piece_ids: Vec::new(),
            resize_filter: "lanczos3".to_string(),
        }
    }

    fn png_bytes(width: u32, height: u32) -> Vec<u8> {
        let mut image = ImageBuffer::from_pixel(width, height, Rgba([0, 0, 0, 0]));
        for (x, y, pixel) in image.enumerate_pixels_mut() {
            *pixel = Rgba([
                ((x * 13 + y * 7) % 255) as u8,
                ((x * 5 + y * 17) % 255) as u8,
                ((x * 19 + y * 3) % 255) as u8,
                255,
            ]);
        }
        let mut bytes = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image)
            .write_to(&mut bytes, ImageFormat::Png)
            .unwrap();
        bytes.into_inner()
    }

    fn animated_gif_bytes() -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut encoder = gif::Encoder::new(&mut bytes, 24, 24, &[]).unwrap();
            encoder.set_repeat(gif::Repeat::Infinite).unwrap();
            for frame_index in 0..8_u8 {
                let mut pixels = Vec::with_capacity(24 * 24 * 4);
                for y in 0..24_u8 {
                    for x in 0..24_u8 {
                        pixels.extend_from_slice(&[
                            x.wrapping_mul(9).wrapping_add(frame_index * 13),
                            y.wrapping_mul(7).wrapping_add(frame_index * 11),
                            frame_index.wrapping_mul(27),
                            255,
                        ]);
                    }
                }
                let mut frame = gif::Frame::from_rgba_speed(24, 24, &mut pixels, 10);
                frame.delay = 6;
                encoder.write_frame(&frame).unwrap();
            }
        }
        bytes
    }

    fn animated_gif_bytes_with_frames(frame_count: u8) -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut encoder = gif::Encoder::new(&mut bytes, 48, 48, &[]).unwrap();
            encoder.set_repeat(gif::Repeat::Infinite).unwrap();
            for frame_index in 0..frame_count {
                let mut pixels = Vec::with_capacity(48 * 48 * 4);
                for y in 0..48_u8 {
                    for x in 0..48_u8 {
                        pixels.extend_from_slice(&[
                            x.wrapping_mul(5).wrapping_add(frame_index.wrapping_mul(3)),
                            y.wrapping_mul(7).wrapping_add(frame_index.wrapping_mul(5)),
                            frame_index.wrapping_mul(11),
                            255,
                        ]);
                    }
                }
                let mut frame = gif::Frame::from_rgba_speed(48, 48, &mut pixels, 10);
                frame.delay = 3;
                encoder.write_frame(&frame).unwrap();
            }
        }
        bytes
    }
}
