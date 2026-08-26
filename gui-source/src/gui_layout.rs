pub const BASE_CLIENT_WIDTH: i32 = 964;
pub const BASE_CLIENT_HEIGHT: i32 = 621;
pub const DEFAULT_DPI: u32 = 96;

pub fn scale_for_dpi(value: i32, dpi: u32) -> i32 {
    let dpi = dpi.max(DEFAULT_DPI);
    ((i64::from(value) * i64::from(dpi) + i64::from(DEFAULT_DPI / 2)) / i64::from(DEFAULT_DPI))
        as i32
}

pub fn to_logical_point(
    physical_x: i32,
    physical_y: i32,
    client_width: i32,
    client_height: i32,
) -> (i32, i32) {
    if client_width <= 0 || client_height <= 0 {
        return (physical_x, physical_y);
    }
    (
        physical_x.saturating_mul(BASE_CLIENT_WIDTH) / client_width,
        physical_y.saturating_mul(BASE_CLIENT_HEIGHT) / client_height,
    )
}

pub fn scale_to_client(value: i32, logical_extent: i32, physical_extent: i32) -> i32 {
    if logical_extent <= 0 || physical_extent <= 0 {
        return value;
    }
    value.saturating_mul(physical_extent) / logical_extent
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supported_dpi_sizes_are_exact() {
        assert_eq!(scale_for_dpi(BASE_CLIENT_WIDTH, 96), 964);
        assert_eq!(scale_for_dpi(BASE_CLIENT_WIDTH, 120), 1205);
        assert_eq!(scale_for_dpi(BASE_CLIENT_WIDTH, 144), 1446);
        assert_eq!(scale_for_dpi(BASE_CLIENT_WIDTH, 192), 1928);
    }

    #[test]
    fn hit_testing_maps_back_to_logical_coordinates() {
        for dpi in [96, 120, 144, 192] {
            let width = scale_for_dpi(BASE_CLIENT_WIDTH, dpi);
            let height = scale_for_dpi(BASE_CLIENT_HEIGHT, dpi);
            let physical_x = scale_to_client(200, BASE_CLIENT_WIDTH, width);
            let physical_y = scale_to_client(240, BASE_CLIENT_HEIGHT, height);
            let (logical_x, logical_y) = to_logical_point(physical_x, physical_y, width, height);
            assert!((199..=200).contains(&logical_x));
            assert!((239..=240).contains(&logical_y));
        }
    }
}
