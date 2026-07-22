use std::io::Cursor;

use crate::error::{AppError, AppResult};
use crate::imaging::import_limits::validate_gif_workload;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GifOutputRepeat {
    Once,
    Infinite,
    Finite(u16),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GifInspection {
    pub frame_count: i64,
    pub loop_mode: String,
    pub loop_count: Option<i64>,
}

pub fn inspect_gif_bytes(bytes: &[u8]) -> Result<GifInspection, String> {
    let mut options = gif::DecodeOptions::new();
    options.set_color_output(gif::ColorOutput::RGBA);
    let mut reader = options
        .read_info(Cursor::new(bytes))
        .map_err(|_| "GIF 파일을 읽을 수 없습니다.".to_string())?;
    let repeat = reader.repeat();
    let has_loop_extension = has_netscape_loop_extension(bytes);
    let width = u32::from(reader.width());
    let height = u32::from(reader.height());
    let mut frame_count = 0_i64;

    while reader
        .read_next_frame()
        .map_err(|_| "GIF 프레임을 읽을 수 없습니다.".to_string())?
        .is_some()
    {
        frame_count += 1;
        validate_gif_workload(width, height, frame_count)?;
    }

    if frame_count == 0 {
        return Err("GIF 프레임을 찾을 수 없습니다.".to_string());
    }

    let (loop_mode, loop_count) = source_loop_metadata(repeat, has_loop_extension);

    Ok(GifInspection {
        frame_count,
        loop_mode,
        loop_count,
    })
}

pub fn output_repeat_for_settings(
    loop_mode: &str,
    loop_count: Option<i64>,
    source_loop_mode: &str,
    source_loop_count: Option<i64>,
) -> AppResult<GifOutputRepeat> {
    match loop_mode {
        "infinite" => Ok(GifOutputRepeat::Infinite),
        "pingpong" => Ok(GifOutputRepeat::Infinite),
        "once" => Ok(GifOutputRepeat::Once),
        "count" => Ok(GifOutputRepeat::Finite(normalized_loop_count(loop_count)?)),
        "preserve" => source_output_repeat(source_loop_mode, source_loop_count),
        _ => Err(AppError::new(
            "validation",
            "지원하지 않는 GIF 반복 설정입니다.",
        )),
    }
}

pub fn is_pingpong_loop_mode(loop_mode: &str) -> bool {
    loop_mode == "pingpong"
}

pub fn pingpong_sequence<T: Clone>(frames: &mut Vec<T>) {
    if frames.len() <= 2 {
        return;
    }

    let reflected = frames[1..frames.len() - 1]
        .iter()
        .rev()
        .cloned()
        .collect::<Vec<_>>();
    frames.extend(reflected);
}

fn source_loop_metadata(repeat: gif::Repeat, has_loop_extension: bool) -> (String, Option<i64>) {
    if !has_loop_extension {
        return ("once".to_string(), None);
    }

    match repeat {
        gif::Repeat::Infinite | gif::Repeat::Finite(0) => ("infinite".to_string(), None),
        gif::Repeat::Finite(count) => ("count".to_string(), Some(i64::from(count))),
    }
}

fn source_output_repeat(
    source_loop_mode: &str,
    source_loop_count: Option<i64>,
) -> AppResult<GifOutputRepeat> {
    match source_loop_mode {
        "infinite" => Ok(GifOutputRepeat::Infinite),
        "once" => Ok(GifOutputRepeat::Once),
        "count" => Ok(GifOutputRepeat::Finite(normalized_loop_count(
            source_loop_count,
        )?)),
        // Older rows may still carry the schema default. For previews, favor a
        // visible continuous animation when the original loop metadata is unknown.
        "preserve" => Ok(GifOutputRepeat::Infinite),
        _ => Err(AppError::new(
            "validation",
            "원본 GIF 반복 정보를 해석할 수 없습니다.",
        )),
    }
}

fn normalized_loop_count(loop_count: Option<i64>) -> AppResult<u16> {
    let count = loop_count.unwrap_or(1);
    if count <= 0 {
        return Err(AppError::new(
            "validation",
            "사용자 지정 반복 횟수는 1 이상이어야 합니다.",
        ));
    }

    Ok(count.min(i64::from(u16::MAX)) as u16)
}

fn has_netscape_loop_extension(bytes: &[u8]) -> bool {
    const NETSCAPE_EXTENSION_HEADER: &[u8] = b"\x21\xFF\x0BNETSCAPE2.0";

    bytes
        .windows(NETSCAPE_EXTENSION_HEADER.len())
        .any(|window| window == NETSCAPE_EXTENSION_HEADER)
}

#[cfg(test)]
mod tests {
    use super::{
        inspect_gif_bytes, output_repeat_for_settings, pingpong_sequence, GifOutputRepeat,
    };

    #[test]
    fn output_repeat_uses_saved_source_loop_for_preserve() {
        assert_eq!(
            output_repeat_for_settings("preserve", None, "infinite", None).unwrap(),
            GifOutputRepeat::Infinite,
        );
        assert_eq!(
            output_repeat_for_settings("preserve", None, "once", None).unwrap(),
            GifOutputRepeat::Once,
        );
        assert_eq!(
            output_repeat_for_settings("preserve", None, "count", Some(3)).unwrap(),
            GifOutputRepeat::Finite(3),
        );
    }

    #[test]
    fn custom_loop_count_is_clamped_to_gif_limit() {
        assert_eq!(
            output_repeat_for_settings("count", Some(99_999), "once", None).unwrap(),
            GifOutputRepeat::Finite(u16::MAX),
        );
    }

    #[test]
    fn inspect_rejects_invalid_gif_bytes() {
        assert!(inspect_gif_bytes(b"not a gif").is_err());
    }

    #[test]
    fn pingpong_sequence_reflects_middle_frames() {
        let mut frames = vec![0, 1, 2, 3];
        pingpong_sequence(&mut frames);
        assert_eq!(frames, vec![0, 1, 2, 3, 2, 1]);
    }

    #[test]
    fn pingpong_loop_outputs_infinite_repeat() {
        assert_eq!(
            output_repeat_for_settings("pingpong", None, "once", None).unwrap(),
            GifOutputRepeat::Infinite,
        );
    }
}
