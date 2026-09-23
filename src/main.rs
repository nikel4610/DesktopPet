#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod asset;
mod config;

use std::{
    ffi::c_void,
    mem::{size_of, zeroed},
    ptr::{null, null_mut},
};

use asset::character_pack::{CharacterPack, default_character_root};
use image::{
    RgbaImage,
    imageops::{FilterType, resize},
};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM},
    Graphics::Gdi::{
        AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION,
        CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC,
        GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint, ReleaseDC,
        SelectObject,
    },
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        Input::KeyboardAndMouse::{ReleaseCapture, SetCapture},
        WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DispatchMessageW, GWLP_USERDATA, GetCursorPos,
            GetMessageW, GetWindowLongPtrW, GetWindowRect, IDC_ARROW, LoadCursorW, MSG,
            PostQuitMessage, RegisterClassW, SW_SHOW, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER,
            SetWindowLongPtrW, SetWindowPos, ShowWindow, TranslateMessage, ULW_ALPHA,
            UpdateLayeredWindow, WM_CAPTURECHANGED, WM_DESTROY, WM_LBUTTONDOWN, WM_LBUTTONUP,
            WM_MOUSEMOVE, WNDCLASSW, WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_POPUP,
        },
    },
};

const WINDOW_CLASS_NAME: &[u16] = &[
    'D' as u16, 'e' as u16, 's' as u16, 'k' as u16, 't' as u16, 'o' as u16, 'p' as u16, 'P' as u16,
    'e' as u16, 't' as u16, 'W' as u16, 'i' as u16, 'n' as u16, 'd' as u16, 'o' as u16, 'w' as u16,
    0,
];

#[derive(Default)]
struct DragState {
    active: bool,
    offset_x: i32,
    offset_y: i32,
    width: i32,
    height: i32,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("Desktop Pet startup failed: {error}");
    }
}

fn run() -> Result<(), String> {
    let character_root = default_character_root()?;
    let pack = CharacterPack::load_first(&character_root)?;
    let idle = pack.resolve_motion("idle")?;
    let first_frame = idle
        .frames
        .first()
        .ok_or_else(|| format!("character pack '{}' has no idle frame", pack.id))?;

    let image = image::open(first_frame)
        .map_err(|error| format!("failed to decode {}: {error}", first_frame.display()))?
        .to_rgba8();
    let image = scale_image(image, pack.config.scale)?;

    let (width, height) = image.dimensions();
    let width = i32::try_from(width).map_err(|_| "character image width is too large")?;
    let height = i32::try_from(height).map_err(|_| "character image height is too large")?;
    let window_title = wide_null(&pack.config.name);

    #[cfg(debug_assertions)]
    {
        let motions = pack.available_motion_names().collect::<Vec<_>>().join(", ");
        eprintln!(
            "Loaded character '{}' ({}) v{} by '{}' from {} | motions: {} | idle: {} fps, loop={} | anchor=({}, {}) | facing={:?}",
            pack.config.name,
            pack.id,
            pack.config.version,
            pack.config.author,
            pack.root.display(),
            motions,
            idle.config.fps,
            idle.config.looped,
            pack.config.anchor_x,
            pack.config.anchor_y,
            pack.config.default_facing
        );
    }

    unsafe {
        let instance = GetModuleHandleW(null());
        if instance.is_null() {
            return Err("GetModuleHandleW failed".into());
        }

        let window_class = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(window_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: instance,
            hIcon: null_mut(),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hbrBackground: null_mut(),
            lpszMenuName: null(),
            lpszClassName: WINDOW_CLASS_NAME.as_ptr(),
        };

        if RegisterClassW(&window_class) == 0 {
            return Err("RegisterClassW failed".into());
        }

        let hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            WINDOW_CLASS_NAME.as_ptr(),
            window_title.as_ptr(),
            WS_POPUP,
            200,
            200,
            width,
            height,
            null_mut(),
            null_mut(),
            instance,
            null_mut(),
        );

        if hwnd.is_null() {
            return Err("CreateWindowExW failed".into());
        }

        let mut drag_state = Box::new(DragState::default());
        SetWindowLongPtrW(
            hwnd,
            GWLP_USERDATA,
            (&mut *drag_state as *mut DragState) as isize,
        );

        update_layered_window(hwnd, image.as_raw(), width, height)?;

        ShowWindow(hwnd, SW_SHOW);

        let mut message: MSG = zeroed();
        loop {
            let result = GetMessageW(&mut message, null_mut(), 0, 0);
            if result == -1 {
                return Err("GetMessageW failed".into());
            }
            if result == 0 {
                break;
            }

            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }

    Ok(())
}

fn scale_image(image: RgbaImage, scale: f32) -> Result<RgbaImage, String> {
    if (scale - 1.0).abs() < f32::EPSILON {
        return Ok(image);
    }

    let width = ((image.width() as f64) * f64::from(scale)).round();
    let height = ((image.height() as f64) * f64::from(scale)).round();

    if !width.is_finite()
        || !height.is_finite()
        || width < 1.0
        || height < 1.0
        || width > f64::from(u32::MAX)
        || height > f64::from(u32::MAX)
    {
        return Err(format!("scale {scale} produces an invalid image size"));
    }

    Ok(resize(
        &image,
        width as u32,
        height as u32,
        FilterType::Nearest,
    ))
}

fn wide_null(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn update_layered_window(hwnd: HWND, rgba: &[u8], width: i32, height: i32) -> Result<(), String> {
    // SAFETY: All handles are created and released within this function, and the DIB buffer
    // is sized to exactly width * height * 4 bytes before it is exposed as a mutable slice.
    unsafe {
        let screen_dc = GetDC(null_mut());
        if screen_dc.is_null() {
            return Err("GetDC failed".into());
        }

        let memory_dc = CreateCompatibleDC(screen_dc);
        if memory_dc.is_null() {
            ReleaseDC(null_mut(), screen_dc);
            return Err("CreateCompatibleDC failed".into());
        }

        let mut bitmap_info: BITMAPINFO = zeroed();
        bitmap_info.bmiHeader = BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            biSizeImage: 0,
            biXPelsPerMeter: 0,
            biYPelsPerMeter: 0,
            biClrUsed: 0,
            biClrImportant: 0,
        };

        let mut bitmap_bits: *mut c_void = null_mut();
        let bitmap = CreateDIBSection(
            memory_dc,
            &bitmap_info,
            DIB_RGB_COLORS,
            &mut bitmap_bits,
            null_mut(),
            0,
        );

        if bitmap.is_null() || bitmap_bits.is_null() {
            DeleteDC(memory_dc);
            ReleaseDC(null_mut(), screen_dc);
            return Err("CreateDIBSection failed".into());
        }

        let previous_object = SelectObject(memory_dc, bitmap as _);

        let pixel_count = (width * height) as usize;
        let destination = std::slice::from_raw_parts_mut(bitmap_bits.cast::<u8>(), pixel_count * 4);

        for (src, dst) in rgba.chunks_exact(4).zip(destination.chunks_exact_mut(4)) {
            let alpha = src[3] as u16;
            dst[0] = ((src[2] as u16 * alpha + 127) / 255) as u8;
            dst[1] = ((src[1] as u16 * alpha + 127) / 255) as u8;
            dst[2] = ((src[0] as u16 * alpha + 127) / 255) as u8;
            dst[3] = src[3];
        }

        let source_position = POINT { x: 0, y: 0 };
        let size = SIZE {
            cx: width,
            cy: height,
        };
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };

        let updated = UpdateLayeredWindow(
            hwnd,
            screen_dc,
            null(),
            &size,
            memory_dc,
            &source_position,
            0,
            &blend,
            ULW_ALPHA,
        );

        SelectObject(memory_dc, previous_object);
        DeleteObject(bitmap as _);
        DeleteDC(memory_dc);
        ReleaseDC(null_mut(), screen_dc);

        if updated == 0 {
            return Err("UpdateLayeredWindow failed".into());
        }

        Ok(())
    }
}

fn clamp_rect_to_bounds(rect: &mut RECT, bounds: RECT) {
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    let bounds_width = bounds.right - bounds.left;
    let bounds_height = bounds.bottom - bounds.top;

    if width >= bounds_width {
        rect.left = bounds.left;
        rect.right = bounds.left + width;
    } else if rect.left < bounds.left {
        rect.left = bounds.left;
        rect.right = bounds.left + width;
    } else if rect.right > bounds.right {
        rect.right = bounds.right;
        rect.left = bounds.right - width;
    }

    if height >= bounds_height {
        rect.top = bounds.top;
        rect.bottom = bounds.top + height;
    } else if rect.top < bounds.top {
        rect.top = bounds.top;
        rect.bottom = bounds.top + height;
    } else if rect.bottom > bounds.bottom {
        rect.bottom = bounds.bottom;
        rect.top = bounds.bottom - height;
    }
}

fn clamp_rect_to_cursor_monitor(rect: &mut RECT, cursor: POINT) {
    // SAFETY: The monitor handle is used only for this lookup and MONITORINFO is initialized
    // with the required cbSize before calling GetMonitorInfoW.
    unsafe {
        let monitor = MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST);
        if monitor.is_null() {
            return;
        }

        let mut monitor_info: MONITORINFO = zeroed();
        monitor_info.cbSize = size_of::<MONITORINFO>() as u32;

        if GetMonitorInfoW(monitor, &mut monitor_info) != 0 {
            clamp_rect_to_bounds(rect, monitor_info.rcMonitor);
        }
    }
}

unsafe fn drag_state(hwnd: HWND) -> *mut DragState {
    // SAFETY: GWLP_USERDATA is initialized with a DragState pointer immediately after
    // CreateWindowExW succeeds and the Box remains alive for the whole message loop.
    unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut DragState }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_LBUTTONDOWN => {
            let state = unsafe { drag_state(hwnd) };
            if !state.is_null() {
                let mut cursor: POINT = unsafe { zeroed() };
                let mut window_rect: RECT = unsafe { zeroed() };

                if unsafe { GetCursorPos(&mut cursor) } != 0
                    && unsafe { GetWindowRect(hwnd, &mut window_rect) } != 0
                {
                    // SAFETY: state points to the DragState owned by run() for the lifetime
                    // of this window.
                    let state = unsafe { &mut *state };
                    state.active = true;
                    state.offset_x = cursor.x - window_rect.left;
                    state.offset_y = cursor.y - window_rect.top;
                    state.width = window_rect.right - window_rect.left;
                    state.height = window_rect.bottom - window_rect.top;

                    unsafe { SetCapture(hwnd) };
                }
            }
            0
        }
        WM_MOUSEMOVE => {
            let state = unsafe { drag_state(hwnd) };
            if !state.is_null() {
                // SAFETY: state points to the DragState owned by run() for the lifetime
                // of this window.
                let state = unsafe { &mut *state };

                if state.active {
                    let mut cursor: POINT = unsafe { zeroed() };
                    if unsafe { GetCursorPos(&mut cursor) } != 0 {
                        let left = cursor.x - state.offset_x;
                        let top = cursor.y - state.offset_y;
                        let mut target_rect = RECT {
                            left,
                            top,
                            right: left + state.width,
                            bottom: top + state.height,
                        };

                        clamp_rect_to_cursor_monitor(&mut target_rect, cursor);

                        unsafe {
                            SetWindowPos(
                                hwnd,
                                null_mut(),
                                target_rect.left,
                                target_rect.top,
                                0,
                                0,
                                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                            )
                        };
                    }
                }
            }
            0
        }
        WM_LBUTTONUP => {
            let state = unsafe { drag_state(hwnd) };
            if !state.is_null() {
                // SAFETY: state points to the DragState owned by run() for the lifetime
                // of this window.
                unsafe { (*state).active = false };
            }
            unsafe { ReleaseCapture() };
            0
        }
        WM_CAPTURECHANGED => {
            let state = unsafe { drag_state(hwnd) };
            if !state.is_null() {
                // SAFETY: state points to the DragState owned by run() for the lifetime
                // of this window.
                unsafe { (*state).active = false };
            }
            0
        }
        WM_DESTROY => {
            // SAFETY: Posting WM_QUIT does not dereference any caller-provided pointer.
            unsafe { PostQuitMessage(0) };
            0
        }
        _ => {
            // SAFETY: Unhandled messages are forwarded with the original Win32 parameters.
            unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_rect_keeps_window_inside_monitor_bounds() {
        let bounds = RECT {
            left: 0,
            top: 0,
            right: 1920,
            bottom: 1080,
        };
        let mut rect = RECT {
            left: 1880,
            top: 1040,
            right: 1980,
            bottom: 1140,
        };

        clamp_rect_to_bounds(&mut rect, bounds);

        assert_eq!(rect.left, 1820);
        assert_eq!(rect.top, 980);
        assert_eq!(rect.right, 1920);
        assert_eq!(rect.bottom, 1080);
    }

    #[test]
    fn clamp_rect_supports_negative_monitor_coordinates() {
        let bounds = RECT {
            left: -1920,
            top: -200,
            right: 0,
            bottom: 880,
        };
        let mut rect = RECT {
            left: -1990,
            top: -250,
            right: -1890,
            bottom: -150,
        };

        clamp_rect_to_bounds(&mut rect, bounds);

        assert_eq!(rect.left, -1920);
        assert_eq!(rect.top, -200);
        assert_eq!(rect.right, -1820);
        assert_eq!(rect.bottom, -100);
    }
}
