use image::{imageops, RgbaImage};

use crate::error::{AppError, AppResult};
use crate::imaging::geometry::{viewport_size, ViewportSize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageTransform {
    pub quarter_turns: i64,
    pub flip_horizontal: bool,
    pub flip_vertical: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceViewportGeometry {
    pub shape: &'static str,
    pub cell_width: i64,
    pub cell_height: i64,
    pub viewport: ViewportSize,
}

impl ImageTransform {
    pub fn new(quarter_turns: i64, flip_horizontal: bool, flip_vertical: bool) -> AppResult<Self> {
        if !(0..=3).contains(&quarter_turns) {
            return Err(AppError::new(
                "validation",
                "회전 값은 0, 90, 180, 270도 중 하나여야 합니다.",
            ));
        }

        if flip_vertical {
            return Ok(Self {
                quarter_turns: (quarter_turns + 2) % 4,
                flip_horizontal: !flip_horizontal,
                flip_vertical: false,
            });
        }

        Ok(Self {
            quarter_turns,
            flip_horizontal,
            flip_vertical: false,
        })
    }

    pub fn is_identity(self) -> bool {
        self.quarter_turns == 0 && !self.flip_horizontal && !self.flip_vertical
    }
}

pub fn source_viewport_geometry(
    output_shape: &str,
    output_cell_width: i64,
    output_cell_height: i64,
    transform: ImageTransform,
) -> AppResult<SourceViewportGeometry> {
    let (shape, cell_width, cell_height) = if transform.quarter_turns % 2 == 0 {
        (
            normalized_shape(output_shape)?,
            output_cell_width,
            output_cell_height,
        )
    } else {
        (
            quarter_turn_source_shape(output_shape)?,
            output_cell_height,
            output_cell_width,
        )
    };
    let viewport = viewport_size(shape, cell_width, cell_height)?;

    Ok(SourceViewportGeometry {
        shape,
        cell_width,
        cell_height,
        viewport,
    })
}

pub fn apply_image_transform(image: RgbaImage, transform: ImageTransform) -> AppResult<RgbaImage> {
    let mut transformed = match transform.quarter_turns {
        0 => image,
        1 => imageops::rotate90(&image),
        2 => imageops::rotate180(&image),
        3 => imageops::rotate270(&image),
        _ => {
            return Err(AppError::new(
                "validation",
                "회전 값은 0, 90, 180, 270도 중 하나여야 합니다.",
            ));
        }
    };

    if transform.flip_horizontal {
        transformed = imageops::flip_horizontal(&transformed);
    }
    if transform.flip_vertical {
        transformed = imageops::flip_vertical(&transformed);
    }

    Ok(transformed)
}

fn normalized_shape(shape: &str) -> AppResult<&'static str> {
    match shape {
        "single" => Ok("single"),
        "horizontal_double" => Ok("horizontal_double"),
        "vertical_double" => Ok("vertical_double"),
        _ => Err(AppError::new(
            "validation",
            "지원하지 않는 아이콘 모양입니다.",
        )),
    }
}

fn quarter_turn_source_shape(output_shape: &str) -> AppResult<&'static str> {
    match output_shape {
        "single" => Ok("single"),
        "horizontal_double" => Ok("vertical_double"),
        "vertical_double" => Ok("horizontal_double"),
        _ => Err(AppError::new(
            "validation",
            "지원하지 않는 아이콘 모양입니다.",
        )),
    }
}

#[cfg(test)]
mod tests {
    use image::{ImageBuffer, Rgba};

    use super::{
        apply_image_transform, source_viewport_geometry, ImageTransform, SourceViewportGeometry,
    };
    use crate::imaging::geometry::ViewportSize;

    fn transform(quarter_turns: i64, flip_horizontal: bool, flip_vertical: bool) -> ImageTransform {
        ImageTransform::new(quarter_turns, flip_horizontal, flip_vertical).unwrap()
    }

    #[test]
    fn odd_rotation_uses_inverse_source_geometry_for_multi_piece_output() {
        assert_eq!(
            source_viewport_geometry("vertical_double", 120, 80, transform(1, false, false),)
                .unwrap(),
            SourceViewportGeometry {
                shape: "horizontal_double",
                cell_width: 80,
                cell_height: 120,
                viewport: ViewportSize {
                    width: 160,
                    height: 120,
                },
            },
        );
    }

    #[test]
    fn quarter_turn_and_output_axis_flips_transform_pixels_deterministically() {
        let image = ImageBuffer::from_fn(2, 3, |x, y| Rgba([(y * 2 + x) as u8, 0, 0, 255]));
        let transformed = apply_image_transform(image, transform(1, true, false)).unwrap();

        assert_eq!((transformed.width(), transformed.height()), (3, 2));
        let values = transformed
            .pixels()
            .map(|pixel| pixel.0[0])
            .collect::<Vec<_>>();
        assert_eq!(values, vec![0, 2, 4, 1, 3, 5]);
    }

    #[test]
    fn all_eight_canonical_states_have_fixed_pixel_orientation() {
        let expected = [
            (0, false, (2, 3), vec![0, 1, 2, 3, 4, 5]),
            (0, true, (2, 3), vec![1, 0, 3, 2, 5, 4]),
            (1, false, (3, 2), vec![4, 2, 0, 5, 3, 1]),
            (1, true, (3, 2), vec![0, 2, 4, 1, 3, 5]),
            (2, false, (2, 3), vec![5, 4, 3, 2, 1, 0]),
            (2, true, (2, 3), vec![4, 5, 2, 3, 0, 1]),
            (3, false, (3, 2), vec![1, 3, 5, 0, 2, 4]),
            (3, true, (3, 2), vec![5, 3, 1, 4, 2, 0]),
        ];

        for (quarter_turns, flip_horizontal, dimensions, values) in expected {
            let image = ImageBuffer::from_fn(2, 3, |x, y| Rgba([(y * 2 + x) as u8, 0, 0, 255]));
            let transformed =
                apply_image_transform(image, transform(quarter_turns, flip_horizontal, false))
                    .unwrap();

            assert_eq!(
                (transformed.width(), transformed.height()),
                dimensions,
                "quarter_turns={quarter_turns}, flip_horizontal={flip_horizontal}",
            );
            assert_eq!(
                transformed
                    .pixels()
                    .map(|pixel| pixel.0[0])
                    .collect::<Vec<_>>(),
                values,
                "quarter_turns={quarter_turns}, flip_horizontal={flip_horizontal}",
            );
        }
    }

    #[test]
    fn invalid_quarter_turn_is_rejected() {
        assert!(ImageTransform::new(-1, false, false).is_err());
        assert!(ImageTransform::new(4, false, false).is_err());
    }

    #[test]
    fn equivalent_flip_representations_are_canonicalized() {
        assert_eq!(
            ImageTransform::new(0, false, true).unwrap(),
            ImageTransform::new(2, true, false).unwrap(),
        );
        assert_eq!(
            ImageTransform::new(0, true, true).unwrap(),
            ImageTransform::new(2, false, false).unwrap(),
        );
    }
}
