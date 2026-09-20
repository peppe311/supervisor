use winit::window::{Icon, Window};

const APP_ICON_PNG: &[u8] = include_bytes!("../assets/supervisor-app-icon.png");

pub(crate) fn window_icon() -> Icon {
    let image = image::load_from_memory(APP_ICON_PNG)
        .expect("The bundled Supervisor icon is valid PNG")
        .into_rgba8();
    let (width, height) = image.dimensions();
    Icon::from_rgba(image.into_raw(), width, height).expect("Supervisor icon dimensions are valid")
}

pub(crate) fn apply_window_icons(window: &Window, scale_factor: f64, theme: &str) {
    #[cfg(windows)]
    {
        use winit::{
            dpi::PhysicalSize,
            platform::windows::{IconExtWindows, WindowExtWindows},
        };

        // ICON_SMALL needs the optical frame at 100% DPI, not a downsample of
        // the large taskbar image. Windows selects/scales a matching ICO frame.
        let size = (16.0 * scale_factor).round().clamp(16.0, 256.0) as u32;
        let resource = if theme == "central_dark" { 3 } else { 2 };
        let small = Icon::from_resource(resource, Some(PhysicalSize::new(size, size)))
            .unwrap_or_else(|_| window_icon());
        window.set_window_icon(Some(small));
        window.set_taskbar_icon(Some(window_icon()));
    }
    #[cfg(not(windows))]
    {
        let _ = (scale_factor, theme);
        window.set_window_icon(Some(window_icon()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supervisor_icon_has_transparency_and_neutral_contrast() {
        let image = image::load_from_memory(APP_ICON_PNG).unwrap().into_rgba8();
        assert_eq!(image.dimensions(), (256, 256));
        assert!(image.pixels().any(|pixel| pixel[3] == 0));
        assert!(image.pixels().any(|pixel| pixel[3] == 255));
        // Only the symbol is opaque: no native tile or perimeter keyline.
        assert_eq!(image.get_pixel(160, 128)[3], 0);
        assert_eq!(image.get_pixel(16, 128)[3], 0);
        assert!(image.pixels().all(|pixel| {
            pixel[3] == 0 || (pixel[0].abs_diff(pixel[1]) <= 3 && pixel[1].abs_diff(pixel[2]) <= 3)
        }));
        let _ = window_icon();
    }

    #[test]
    fn native_and_interface_brand_assets_share_one_source() {
        let svg = include_str!("../assets/supervisor-mark.svg");
        assert!(svg.contains("data-supervisor-logo=\"true\""));
        assert_eq!(svg.matches("<path ").count(), 2);
        assert!(!svg.contains("brand-signal"));
        let generator = include_str!("../scripts/build-brand-assets.ps1");
        assert!(generator.contains("supervisor-mark.svg"));
        assert!(generator.contains("themes.css"));
        let icon = include_bytes!("../assets/supervisor-app-icon.ico");
        assert_eq!(&icon[..6], &[0, 0, 1, 0, 7, 0]);
        for (index, size) in [16, 24, 32, 48, 64, 128, 256].into_iter().enumerate() {
            let entry = &icon[6 + index * 16..6 + (index + 1) * 16];
            let length = u32::from_le_bytes(entry[8..12].try_into().unwrap()) as usize;
            let offset = u32::from_le_bytes(entry[12..16].try_into().unwrap()) as usize;
            let frame = image::load_from_memory(&icon[offset..offset + length])
                .unwrap()
                .into_rgba8();
            assert_eq!(frame.dimensions(), (size, size));
            assert_eq!(entry[0] as u32, size % 256);
            assert!(frame.pixels().any(|pixel| pixel[3] == 0));
            if size == 16 {
                // The two narrow openings must survive the native rasterizer.
                assert!(frame.get_pixel(10, 8)[3] < 40);
                assert!(frame.get_pixel(4, 10)[3] < 40);
                assert!(frame.get_pixel(8, 8)[3] > 220);
            }
        }
    }

    #[cfg(windows)]
    #[test]
    fn windows_can_load_both_native_icon_roles_from_the_embedded_resource() {
        use winit::{dpi::PhysicalSize, platform::windows::IconExtWindows};
        for resource in [1, 2, 3] {
            for size in [16, 24, 32, 48, 256] {
                assert!(Icon::from_resource(resource, Some(PhysicalSize::new(size, size))).is_ok());
            }
        }
    }
}
