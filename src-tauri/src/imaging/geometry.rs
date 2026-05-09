use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Copy, PartialEq)]
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

    match shape {
        "single" => Ok(ViewportSize {
            width: cell_width,
            height: cell_height,
        }),
        "horizontal_double" => Ok(ViewportSize {
            width: cell_width * 2,
            height: cell_height,
        }),
        "vertical_double" => Ok(ViewportSize {
            width: cell_width,
            height: cell_height * 2,
        }),
        _ => Err(AppError::new(
            "validation",
            "지원하지 않는 아이콘 모양입니다.",
        )),
    }
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
    }
}
