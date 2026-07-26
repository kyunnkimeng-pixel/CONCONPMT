use crate::error::{AppError, AppResult};
use crate::imaging::import_limits::validate_import_dimensions;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViewportSize {
    pub width: i64,
    pub height: i64,
}

pub fn viewport_size(shape: &str, cell_width: i64, cell_height: i64) -> AppResult<ViewportSize> {
    if cell_width <= 0 || cell_height <= 0 {
        return Err(AppError::new(
            "validation",
            "셀 크기는 1px 이상이어야 합니다.",
        ));
    }

    let viewport = match shape {
        "single" => ViewportSize {
            width: cell_width,
            height: cell_height,
        },
        "horizontal_double" => ViewportSize {
            width: cell_width.checked_mul(2).ok_or_else(|| {
                AppError::new("validation", "가로 2칸 viewport 크기가 너무 큽니다.")
            })?,
            height: cell_height,
        },
        "vertical_double" => ViewportSize {
            width: cell_width,
            height: cell_height.checked_mul(2).ok_or_else(|| {
                AppError::new("validation", "세로 2칸 viewport 크기가 너무 큽니다.")
            })?,
        },
        _ => {
            return Err(AppError::new(
                "validation",
                "지원하지 않는 아이콘 모양입니다.",
            ));
        }
    };
    let width = u32::try_from(viewport.width)
        .map_err(|_| AppError::new("validation", "viewport 너비가 올바르지 않습니다."))?;
    let height = u32::try_from(viewport.height)
        .map_err(|_| AppError::new("validation", "viewport 높이가 올바르지 않습니다."))?;
    validate_import_dimensions(width, height)?;
    Ok(viewport)
}

pub fn piece_roles(shape: &str) -> AppResult<&'static [&'static str]> {
    match shape {
        "single" => Ok(&["single"]),
        "horizontal_double" => Ok(&["left", "right"]),
        "vertical_double" => Ok(&["top", "bottom"]),
        _ => Err(AppError::new(
            "validation",
            "지원하지 않는 아이콘 모양입니다.",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::{piece_roles, viewport_size, ViewportSize};

    #[test]
    fn shape_maps_to_viewport_size() {
        assert_eq!(
            viewport_size("single", 200, 160).unwrap(),
            ViewportSize {
                width: 200,
                height: 160,
            },
        );
        assert_eq!(
            viewport_size("horizontal_double", 200, 160).unwrap(),
            ViewportSize {
                width: 400,
                height: 160,
            },
        );
        assert_eq!(
            viewport_size("vertical_double", 200, 160).unwrap(),
            ViewportSize {
                width: 200,
                height: 320,
            },
        );
    }

    #[test]
    fn shape_maps_to_piece_roles() {
        assert_eq!(piece_roles("single").unwrap(), &["single"]);
        assert_eq!(
            piece_roles("horizontal_double").unwrap(),
            &["left", "right"]
        );
        assert_eq!(piece_roles("vertical_double").unwrap(), &["top", "bottom"]);
    }

    #[test]
    fn invalid_cell_size_is_rejected() {
        assert!(viewport_size("single", 0, 200).is_err());
        assert!(viewport_size("single", 200, -1).is_err());
        assert!(viewport_size("horizontal_double", i64::MAX, 1).is_err());
        assert!(viewport_size("vertical_double", 1, i64::MAX).is_err());
        assert!(viewport_size("horizontal_double", 8_000, 1).is_err());
    }
}
