use image::{DynamicImage, RgbImage, codecs::jpeg::JpegEncoder, imageops::FilterType};

use crate::window_runtime::WindowCaptureTarget;

const MAX_CAPTURE_PIXELS: u64 = 34_000_000;
const MAX_PREVIEW_WIDTH: u32 = 1_440;
const MAX_PREVIEW_HEIGHT: u32 = 900;
const MAX_JPEG_BYTES: usize = 900_000;

#[derive(Clone, Debug)]
pub(crate) struct CapturedWindowImage {
    pub(crate) window_id: String,
    pub(crate) title: String,
    pub(crate) pid: u32,
    pub(crate) owned: bool,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) jpeg: Vec<u8>,
}

pub(crate) fn capture_window(target: WindowCaptureTarget) -> Result<CapturedWindowImage, String> {
    #[cfg(not(windows))]
    {
        let _ = target;
        Err("Window capture is available only on Windows".to_owned())
    }

    #[cfg(windows)]
    {
        capture_window_windows(target)
    }
}

#[cfg(windows)]
fn capture_window_windows(target: WindowCaptureTarget) -> Result<CapturedWindowImage, String> {
    use std::{
        thread,
        time::{Duration, Instant},
    };

    use windows::{
        Graphics::{
            Capture::{Direct3D11CaptureFramePool, GraphicsCaptureItem, GraphicsCaptureSession},
            DirectX::DirectXPixelFormat,
            Imaging::{BitmapBufferAccessMode, BitmapPixelFormat, SoftwareBitmap},
        },
        Win32::{
            Foundation::HMODULE,
            Graphics::{
                Direct3D::{D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP},
                Direct3D11::{
                    D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_SDK_VERSION, D3D11CreateDevice,
                    ID3D11Device,
                },
                Dxgi::{IDXGIAdapter, IDXGIDevice},
            },
            System::WinRT::{
                Direct3D11::CreateDirect3D11DeviceFromDXGIDevice,
                Graphics::Capture::IGraphicsCaptureItemInterop, IMemoryBufferByteAccess,
                RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize,
            },
        },
        core::{Interface, factory},
    };

    crate::window_runtime::validate_capture_target(&target)?;
    if !GraphicsCaptureSession::IsSupported()
        .map_err(|error| format!("Could not query Windows Graphics Capture support: {error}"))?
    {
        return Err("Windows Graphics Capture is not supported on this system".to_owned());
    }

    unsafe { RoInitialize(RO_INIT_MULTITHREADED) }
        .map_err(|error| format!("Could not initialize Windows Graphics Capture: {error}"))?;
    struct RoGuard;
    impl Drop for RoGuard {
        fn drop(&mut self) {
            unsafe { RoUninitialize() };
        }
    }
    let _ro_guard = RoGuard;

    let interop = factory::<GraphicsCaptureItem, IGraphicsCaptureItemInterop>()
        .map_err(|error| format!("Could not open the Windows capture factory: {error}"))?;
    let item: GraphicsCaptureItem = unsafe {
        interop.CreateForWindow(windows::Win32::Foundation::HWND(
            target.native_token as *mut core::ffi::c_void,
        ))
    }
    .map_err(|error| format!("Could not select the requested window for capture: {error}"))?;
    let size = item
        .Size()
        .map_err(|error| format!("Could not read the capture size: {error}"))?;
    if size.Width <= 0 || size.Height <= 0 {
        return Err("The selected window has no capturable content".to_owned());
    }
    let pixels = (size.Width as u64).saturating_mul(size.Height as u64);
    if pixels > MAX_CAPTURE_PIXELS {
        return Err("The selected window exceeds the local capture-size limit".to_owned());
    }

    let mut native_device: Option<ID3D11Device> = None;
    let create_device = |driver_type, output: *mut Option<ID3D11Device>| unsafe {
        D3D11CreateDevice(
            None::<&IDXGIAdapter>,
            driver_type,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            None,
            D3D11_SDK_VERSION,
            Some(output),
            None,
            None,
        )
    };
    if create_device(D3D_DRIVER_TYPE_HARDWARE, &raw mut native_device).is_err() {
        create_device(D3D_DRIVER_TYPE_WARP, &raw mut native_device)
            .map_err(|error| format!("Could not create a Direct3D capture device: {error}"))?;
    }
    let native_device = native_device
        .ok_or_else(|| "Windows did not return a Direct3D capture device".to_owned())?;
    let dxgi_device: IDXGIDevice = native_device
        .cast()
        .map_err(|error| format!("Could not obtain the DXGI capture device: {error}"))?;
    let inspectable = unsafe { CreateDirect3D11DeviceFromDXGIDevice(&dxgi_device) }
        .map_err(|error| format!("Could not create the WinRT capture device: {error}"))?;
    let device: windows::Graphics::DirectX::Direct3D11::IDirect3DDevice = inspectable
        .cast()
        .map_err(|error| format!("Could not expose the WinRT capture device: {error}"))?;

    let frame_pool = Direct3D11CaptureFramePool::CreateFreeThreaded(
        &device,
        DirectXPixelFormat::B8G8R8A8UIntNormalized,
        1,
        size,
    )
    .map_err(|error| format!("Could not create the Windows capture frame pool: {error}"))?;
    let session = frame_pool
        .CreateCaptureSession(&item)
        .map_err(|error| format!("Could not create the Windows capture session: {error}"))?;
    let _ = session.SetIsCursorCaptureEnabled(false);
    session
        .StartCapture()
        .map_err(|error| format!("Could not start Windows Graphics Capture: {error}"))?;

    let deadline = Instant::now() + Duration::from_secs(3);
    let frame = loop {
        crate::window_runtime::ensure_default_input_desktop()?;
        match frame_pool.TryGetNextFrame() {
            Ok(frame) => break frame,
            Err(error) if Instant::now() >= deadline => {
                let _ = session.Close();
                let _ = frame_pool.Close();
                return Err(format!(
                    "Windows did not provide a capture frame in time: {error}"
                ));
            }
            Err(_) => {}
        }
        thread::sleep(Duration::from_millis(20));
    };

    let content_size = frame
        .ContentSize()
        .map_err(|error| format!("Could not read the captured content size: {error}"))?;
    let surface = frame
        .Surface()
        .map_err(|error| format!("Could not read the captured Direct3D surface: {error}"))?;
    let bitmap = SoftwareBitmap::CreateCopyFromSurfaceAsync(&surface)
        .and_then(|operation| operation.get())
        .map_err(|error| {
            format!("Could not copy the captured frame to protected CPU memory: {error}")
        })?;
    let bitmap = if bitmap.BitmapPixelFormat().ok() == Some(BitmapPixelFormat::Bgra8) {
        bitmap
    } else {
        SoftwareBitmap::Convert(&bitmap, BitmapPixelFormat::Bgra8)
            .map_err(|error| format!("Could not normalize the captured pixel format: {error}"))?
    };
    let bitmap_width = bitmap
        .PixelWidth()
        .map_err(|error| format!("Could not read the captured bitmap width: {error}"))?;
    let bitmap_height = bitmap
        .PixelHeight()
        .map_err(|error| format!("Could not read the captured bitmap height: {error}"))?;
    let width = content_size.Width.min(bitmap_width).max(0) as u32;
    let height = content_size.Height.min(bitmap_height).max(0) as u32;
    if width == 0 || height == 0 {
        return Err("Windows returned an empty capture frame".to_owned());
    }

    let buffer = bitmap
        .LockBuffer(BitmapBufferAccessMode::Read)
        .map_err(|error| format!("Could not lock the captured bitmap: {error}"))?;
    let plane = buffer
        .GetPlaneDescription(0)
        .map_err(|error| format!("Could not read the captured bitmap layout: {error}"))?;
    let reference = buffer
        .CreateReference()
        .map_err(|error| format!("Could not reference the captured bitmap memory: {error}"))?;
    let byte_access: IMemoryBufferByteAccess = reference
        .cast()
        .map_err(|error| format!("Could not access the captured bitmap memory: {error}"))?;
    let mut pointer = std::ptr::null_mut();
    let mut capacity = 0_u32;
    unsafe { byte_access.GetBuffer(&raw mut pointer, &raw mut capacity) }
        .map_err(|error| format!("Could not map the captured bitmap memory: {error}"))?;
    if pointer.is_null() || plane.StartIndex < 0 || plane.Stride == 0 {
        return Err("Windows returned an invalid captured bitmap layout".to_owned());
    }

    let start = plane.StartIndex as usize;
    let stride = plane.Stride.unsigned_abs() as usize;
    let row_bytes = width as usize * 4;
    if stride < row_bytes {
        return Err("Windows returned a captured bitmap with an invalid stride".to_owned());
    }
    let capacity = capacity as usize;
    let mut rgb = Vec::with_capacity(width as usize * height as usize * 3);
    for row in 0..height as usize {
        let source_row = if plane.Stride > 0 {
            row
        } else {
            height as usize - 1 - row
        };
        let offset = start.saturating_add(source_row.saturating_mul(stride));
        let end = offset.saturating_add(row_bytes);
        if end > capacity {
            return Err("The captured bitmap exceeded its validated memory bounds".to_owned());
        }
        let bytes = unsafe { std::slice::from_raw_parts(pointer.add(offset), row_bytes) };
        for pixel in bytes.chunks_exact(4) {
            rgb.extend_from_slice(&[pixel[2], pixel[1], pixel[0]]);
        }
    }

    let image = RgbImage::from_raw(width, height, rgb)
        .ok_or_else(|| "Could not construct the bounded capture image".to_owned())?;
    let (width, height, jpeg) = encode_bounded_jpeg(image)?;

    let _ = buffer.Close();
    let _ = bitmap.Close();
    let _ = frame.Close();
    let _ = session.Close();
    let _ = frame_pool.Close();

    Ok(CapturedWindowImage {
        window_id: target.window_id,
        title: target.title,
        pid: target.pid,
        owned: target.owned,
        width,
        height,
        jpeg,
    })
}

fn encode_bounded_jpeg(image: RgbImage) -> Result<(u32, u32, Vec<u8>), String> {
    let (target_width, target_height) = bounded_dimensions(
        image.width(),
        image.height(),
        MAX_PREVIEW_WIDTH,
        MAX_PREVIEW_HEIGHT,
    );
    let mut image = if (target_width, target_height) == image.dimensions() {
        image
    } else {
        DynamicImage::ImageRgb8(image)
            .resize_exact(target_width, target_height, FilterType::Triangle)
            .to_rgb8()
    };

    for (quality, scale_percent) in [(76, 100_u32), (58, 100), (48, 82), (40, 68)] {
        if scale_percent < 100 {
            let width = (image.width() * scale_percent / 100).max(1);
            let height = (image.height() * scale_percent / 100).max(1);
            image = DynamicImage::ImageRgb8(image)
                .resize_exact(width, height, FilterType::Triangle)
                .to_rgb8();
        }
        let mut jpeg = Vec::new();
        JpegEncoder::new_with_quality(&mut jpeg, quality)
            .encode(
                image.as_raw(),
                image.width(),
                image.height(),
                image::ExtendedColorType::Rgb8,
            )
            .map_err(|error| format!("Could not encode the captured preview: {error}"))?;
        if jpeg.len() <= MAX_JPEG_BYTES {
            return Ok((image.width(), image.height(), jpeg));
        }
    }
    Err("The captured preview could not be compressed within the local limit".to_owned())
}

fn bounded_dimensions(width: u32, height: u32, max_width: u32, max_height: u32) -> (u32, u32) {
    if width <= max_width && height <= max_height {
        return (width, height);
    }
    let width_ratio = max_width as f64 / width.max(1) as f64;
    let height_ratio = max_height as f64 / height.max(1) as f64;
    let ratio = width_ratio.min(height_ratio);
    (
        (width as f64 * ratio).round().max(1.0) as u32,
        (height as f64 * ratio).round().max(1.0) as u32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_dimensions_preserve_aspect_ratio_and_limits() {
        assert_eq!(bounded_dimensions(800, 600, 1_440, 900), (800, 600));
        assert_eq!(bounded_dimensions(3_840, 2_160, 1_440, 900), (1_440, 810));
        assert_eq!(bounded_dimensions(1_000, 2_000, 1_440, 900), (450, 900));
    }

    #[test]
    fn jpeg_encoder_is_bounded() {
        let image = RgbImage::from_pixel(2_000, 1_200, image::Rgb([20, 80, 160]));
        let (width, height, jpeg) = encode_bounded_jpeg(image).unwrap();
        assert!(width <= MAX_PREVIEW_WIDTH);
        assert!(height <= MAX_PREVIEW_HEIGHT);
        assert!(jpeg.len() <= MAX_JPEG_BYTES);
        assert_eq!(&jpeg[..2], &[0xFF, 0xD8]);
    }
}
