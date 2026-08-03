use image::RgbaImage;

const MAX_SAMPLED_PIXELS: u64 = 65_536;
const MIN_DIMENSION: u32 = 16;
const MIN_NEUTRAL_COVERAGE_PERCENT: u64 = 35;
const MIN_CLASS_BALANCE_PERCENT: u64 = 15;
const MIN_PATTERN_SPAN_PERCENT: u64 = 70;
const MIN_PEAK_DISTANCE: usize = 20;
const MAX_CLASS_LUMA_DISTANCE: usize = 24;
const MAX_NEUTRAL_CHROMA: u8 = 18;
const MIN_PERIODIC_PAIR_PERCENT: u64 = 5;
const MIN_PERIODIC_PAIRS: u64 = 32;
const MIN_OPPOSITE_MATCH_PERCENT: u64 = 84;
const MIN_REPEAT_MATCH_PERCENT: u64 = 80;
const MAX_TILE_SIZE: u32 = 64;

/// Detects a high-confidence, painted gray transparency checker.
///
/// The detector deliberately ignores non-opaque pixels. A transparency viewer's
/// checker is therefore never present in the analyzed pixel data. To avoid
/// flagging small checkered details on a character, both neutral colors must
/// cover a large part of the canvas, span most of both axes, and repeat with a
/// square alternating period in both directions.
pub(crate) fn has_high_confidence_painted_checker(image: &RgbaImage) -> bool {
    let width = image.width();
    let height = image.height();
    if width < MIN_DIMENSION || height < MIN_DIMENSION {
        return false;
    }

    let stride = sampling_stride(width, height);
    let mut histogram = [0_u64; 256];
    let mut sampled = 0_u64;
    for y in (0..height).step_by(stride as usize) {
        for x in (0..width).step_by(stride as usize) {
            sampled += 1;
            if let Some(luma) = neutral_opaque_luma(image.get_pixel(x, y).0) {
                histogram[usize::from(luma)] += 1;
            }
        }
    }
    if sampled < MIN_PERIODIC_PAIRS {
        return false;
    }

    let Some((first_peak, second_peak)) = dominant_separated_luma_peaks(&histogram) else {
        return false;
    };
    let mut classified = 0_u64;
    let mut first_count = 0_u64;
    let mut second_count = 0_u64;
    let mut min_x = width;
    let mut min_y = height;
    let mut max_x = 0_u32;
    let mut max_y = 0_u32;
    for y in (0..height).step_by(stride as usize) {
        for x in (0..width).step_by(stride as usize) {
            let Some(class) = checker_class(image, x, y, first_peak, second_peak) else {
                continue;
            };
            classified += 1;
            if class == 0 {
                first_count += 1;
            } else {
                second_count += 1;
            }
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
    }

    if classified.saturating_mul(100) < sampled.saturating_mul(MIN_NEUTRAL_COVERAGE_PERCENT)
        || first_count.min(second_count).saturating_mul(100)
            < classified.saturating_mul(MIN_CLASS_BALANCE_PERCENT)
    {
        return false;
    }
    let span_width = max_x.saturating_sub(min_x).saturating_add(stride);
    let span_height = max_y.saturating_sub(min_y).saturating_add(stride);
    if u64::from(span_width).saturating_mul(100)
        < u64::from(width).saturating_mul(MIN_PATTERN_SPAN_PERCENT)
        || u64::from(span_height).saturating_mul(100)
            < u64::from(height).saturating_mul(MIN_PATTERN_SPAN_PERCENT)
    {
        return false;
    }

    let minimum_pairs = MIN_PERIODIC_PAIRS.max(
        classified
            .saturating_mul(MIN_PERIODIC_PAIR_PERCENT)
            .div_ceil(100),
    );
    let max_tile = MAX_TILE_SIZE.min(width / 2).min(height / 2);
    (2..=max_tile).any(|tile| {
        checker_period_matches(image, first_peak, second_peak, tile, stride, minimum_pairs)
    })
}

fn sampling_stride(width: u32, height: u32) -> u32 {
    let pixels = u64::from(width).saturating_mul(u64::from(height));
    if pixels <= MAX_SAMPLED_PIXELS {
        return 1;
    }
    let mut stride = 1_u32;
    while pixels.div_ceil(u64::from(stride).saturating_mul(u64::from(stride))) > MAX_SAMPLED_PIXELS
    {
        stride = stride.saturating_add(1);
    }
    stride
}

fn neutral_opaque_luma(pixel: [u8; 4]) -> Option<u8> {
    if pixel[3] < 250 {
        return None;
    }
    let minimum = pixel[0].min(pixel[1]).min(pixel[2]);
    let maximum = pixel[0].max(pixel[1]).max(pixel[2]);
    if maximum.saturating_sub(minimum) > MAX_NEUTRAL_CHROMA {
        return None;
    }
    let luma =
        (u32::from(pixel[0]) * 77 + u32::from(pixel[1]) * 150 + u32::from(pixel[2]) * 29) >> 8;
    Some(u8::try_from(luma).unwrap_or(u8::MAX))
}

fn dominant_separated_luma_peaks(histogram: &[u64; 256]) -> Option<(usize, usize)> {
    let smoothed = std::array::from_fn::<_, 256, _>(|index| {
        let start = index.saturating_sub(3);
        let end = (index + 3).min(255);
        histogram[start..=end].iter().copied().sum::<u64>()
    });
    let first = smoothed
        .iter()
        .enumerate()
        .max_by_key(|(_, count)| *count)?
        .0;
    let second = smoothed
        .iter()
        .enumerate()
        .filter(|(index, _)| index.abs_diff(first) >= MIN_PEAK_DISTANCE)
        .max_by_key(|(_, count)| *count)?
        .0;
    (smoothed[first] > 0 && smoothed[second] > 0).then_some((first, second))
}

fn checker_class(
    image: &RgbaImage,
    x: u32,
    y: u32,
    first_peak: usize,
    second_peak: usize,
) -> Option<u8> {
    let luma = usize::from(neutral_opaque_luma(image.get_pixel(x, y).0)?);
    let first_distance = luma.abs_diff(first_peak);
    let second_distance = luma.abs_diff(second_peak);
    if first_distance.min(second_distance) > MAX_CLASS_LUMA_DISTANCE {
        return None;
    }
    Some(u8::from(second_distance < first_distance))
}

fn checker_period_matches(
    image: &RgbaImage,
    first_peak: usize,
    second_peak: usize,
    tile: u32,
    stride: u32,
    minimum_pairs: u64,
) -> bool {
    let mut horizontal_opposite = PairScore::default();
    let mut vertical_opposite = PairScore::default();
    let mut horizontal_repeat = PairScore::default();
    let mut vertical_repeat = PairScore::default();
    let double_tile = tile.saturating_mul(2);

    for y in (0..image.height()).step_by(stride as usize) {
        for x in (0..image.width()).step_by(stride as usize) {
            let Some(base) = checker_class(image, x, y, first_peak, second_peak) else {
                continue;
            };
            if let Some(target_x) = x.checked_add(tile).filter(|x| *x < image.width()) {
                horizontal_opposite.record(
                    base,
                    checker_class(image, target_x, y, first_peak, second_peak),
                    true,
                );
            }
            if let Some(target_y) = y.checked_add(tile).filter(|y| *y < image.height()) {
                vertical_opposite.record(
                    base,
                    checker_class(image, x, target_y, first_peak, second_peak),
                    true,
                );
            }
            if let Some(target_x) = x.checked_add(double_tile).filter(|x| *x < image.width()) {
                horizontal_repeat.record(
                    base,
                    checker_class(image, target_x, y, first_peak, second_peak),
                    false,
                );
            }
            if let Some(target_y) = y.checked_add(double_tile).filter(|y| *y < image.height()) {
                vertical_repeat.record(
                    base,
                    checker_class(image, x, target_y, first_peak, second_peak),
                    false,
                );
            }
        }
    }

    horizontal_opposite.passes(minimum_pairs, MIN_OPPOSITE_MATCH_PERCENT)
        && vertical_opposite.passes(minimum_pairs, MIN_OPPOSITE_MATCH_PERCENT)
        && horizontal_repeat.passes(minimum_pairs, MIN_REPEAT_MATCH_PERCENT)
        && vertical_repeat.passes(minimum_pairs, MIN_REPEAT_MATCH_PERCENT)
}

#[derive(Debug, Default)]
struct PairScore {
    compared: u64,
    matched: u64,
}

impl PairScore {
    fn record(&mut self, source: u8, target: Option<u8>, expect_opposite: bool) {
        let Some(target) = target else {
            return;
        };
        self.compared += 1;
        if (source != target) == expect_opposite {
            self.matched += 1;
        }
    }

    fn passes(&self, minimum_pairs: u64, minimum_percent: u64) -> bool {
        self.compared >= minimum_pairs
            && self.matched.saturating_mul(100) >= self.compared.saturating_mul(minimum_percent)
    }
}

#[cfg(test)]
mod tests {
    use image::{ImageBuffer, Rgba};

    use super::has_high_confidence_painted_checker;

    #[test]
    fn detects_large_painted_checker_inside_a_real_alpha_border() {
        let image = ImageBuffer::from_fn(200, 200, |x, y| {
            if x < 10 || y < 10 || x >= 190 || y >= 190 {
                Rgba([0, 0, 0, 0])
            } else if (x / 8 + y / 8) % 2 == 0 {
                Rgba([238, 238, 238, 255])
            } else {
                Rgba([190, 190, 190, 255])
            }
        });

        assert!(has_high_confidence_painted_checker(&image));
    }

    #[test]
    fn ignores_real_transparency_and_a_colored_character() {
        let image = ImageBuffer::from_fn(200, 200, |x, y| {
            if (45..155).contains(&x) && (30..170).contains(&y) {
                Rgba([40, 120, 220, 255])
            } else {
                Rgba([0, 0, 0, 0])
            }
        });

        assert!(!has_high_confidence_painted_checker(&image));
    }

    #[test]
    fn ignores_small_checkered_character_detail_and_full_canvas_stripes() {
        let small_detail = ImageBuffer::from_fn(200, 200, |x, y| {
            if (70..130).contains(&x) && (70..130).contains(&y) {
                if (x / 6 + y / 6) % 2 == 0 {
                    Rgba([235, 235, 235, 255])
                } else {
                    Rgba([185, 185, 185, 255])
                }
            } else {
                Rgba([0, 0, 0, 0])
            }
        });
        let stripes = ImageBuffer::from_fn(200, 200, |x, _| {
            if (x / 8) % 2 == 0 {
                Rgba([238, 238, 238, 255])
            } else {
                Rgba([190, 190, 190, 255])
            }
        });

        assert!(!has_high_confidence_painted_checker(&small_detail));
        assert!(!has_high_confidence_painted_checker(&stripes));
    }
}
