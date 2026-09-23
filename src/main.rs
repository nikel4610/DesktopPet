#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod animation;
mod asset;
mod behavior;
mod config;

use std::{
    ffi::c_void,
    mem::{size_of, zeroed},
    ptr::{null, null_mut},
    time::Instant,
};

use animation::{Animation, Reaction};
use asset::character_pack::{CharacterPack, default_character_root};
use image::{
    RgbaImage,
    imageops::{FilterType, flip_horizontal_in_place, resize},
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
        Input::KeyboardAndMouse::{GetDoubleClickTime, ReleaseCapture, SetCapture},
        WindowsAndMessaging::{
            CS_DBLCLKS, CreateWindowExW, DefWindowProcW, DispatchMessageW, GWLP_USERDATA,
            GetCursorPos, GetMessageW, GetSystemMetrics, GetWindowLongPtrW, GetWindowRect,
            IDC_ARROW, KillTimer, LoadCursorW, MSG, PostQuitMessage, RegisterClassW, SM_CXDRAG,
            SM_CYDRAG, SW_SHOW, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, SetTimer,
            SetWindowLongPtrW, SetWindowPos, ShowWindow, TranslateMessage, ULW_ALPHA,
            UpdateLayeredWindow, WM_CAPTURECHANGED, WM_DESTROY, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN,
            WM_LBUTTONUP, WM_MOUSEMOVE, WM_TIMER, WNDCLASSW, WS_EX_LAYERED, WS_EX_TOOLWINDOW,
            WS_EX_TOPMOST, WS_POPUP,
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
    moved: bool,
    double_click: bool,
    start_x: i32,
    start_y: i32,
    offset_x: i32,
    offset_y: i32,
    width: i32,
    height: i32,
}

struct PetState {
    drag: DragState,
    click_pending: bool,
    animation: Animation,
    scale: f32,
    width: i32,
    height: i32,
}

const ANIMATION_TIMER: usize = 1;
const CLICK_TIMER: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReleaseAction {
    Click,
    DoubleClick,
    Drop,
}

fn release_action(drag: &DragState) -> ReleaseAction {
    if drag.moved {
        ReleaseAction::Drop
    } else if drag.double_click {
        ReleaseAction::DoubleClick
    } else {
        ReleaseAction::Click
    }
}

fn passed_drag_threshold(
    drag: &DragState,
    cursor: POINT,
    threshold_x: i32,
    threshold_y: i32,
) -> bool {
    (i64::from(cursor.x) - i64::from(drag.start_x)).abs() >= i64::from(threshold_x.max(1))
        || (i64::from(cursor.y) - i64::from(drag.start_y)).abs() >= i64::from(threshold_y.max(1))
}

fn main() {
    if let Err(error) = run() {
        eprintln!("Desktop Pet startup failed: {error}");
    }
}

fn run() -> Result<(), String> {
    let character_root = default_character_root()?;
    let pack = CharacterPack::load_first(&character_root)?;
    let animation = Animation::new(&pack, Instant::now())?;
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
            style: CS_DBLCLKS,
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

        let mut state = Box::new(PetState {
            drag: DragState::default(),
            click_pending: false,
            animation,
            scale: pack.config.scale,
            width,
            height,
        });
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, (&mut *state as *mut PetState) as isize);

        update_layered_window(hwnd, image.as_raw(), width, height)?;

        ShowWindow(hwnd, SW_SHOW);
        if SetTimer(hwnd, ANIMATION_TIMER, state.animation.interval_ms(), None) == 0 {
            return Err("SetTimer failed".into());
        }

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

        // A zero-alpha pixel is also transparent to mouse hit testing on a layered window.
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

fn render_frame(hwnd: HWND, state: &PetState) -> Result<(), String> {
    let frame = state.animation.frame();
    let image = image::open(frame)
        .map_err(|error| format!("failed to decode {}: {error}", frame.display()))?
        .to_rgba8();
    let mut image = scale_image(image, state.scale)?;
    if image.width() != state.width as u32 || image.height() != state.height as u32 {
        return Err(format!(
            "frame size differs from first idle frame: {}",
            frame.display()
        ));
    }
    if state.animation.should_flip() {
        flip_horizontal_in_place(&mut image);
    }
    update_layered_window(hwnd, image.as_raw(), state.width, state.height)
}

fn move_walk(hwnd: HWND, dx: i32) -> Result<bool, String> {
    unsafe {
        let mut rect: RECT = zeroed();
        if GetWindowRect(hwnd, &mut rect) == 0 {
            return Err("GetWindowRect failed".into());
        }
        let center = POINT {
            x: rect.left + (rect.right - rect.left) / 2,
            y: rect.top + (rect.bottom - rect.top) / 2,
        };
        let monitor = MonitorFromPoint(center, MONITOR_DEFAULTTONEAREST);
        let mut info: MONITORINFO = zeroed();
        info.cbSize = size_of::<MONITORINFO>() as u32;
        if monitor.is_null() || GetMonitorInfoW(monitor, &mut info) == 0 {
            return Err("GetMonitorInfoW failed".into());
        }
        let max_left = (info.rcMonitor.right - (rect.right - rect.left)).max(info.rcMonitor.left);
        let wanted = rect.left.saturating_add(dx);
        let left = wanted.clamp(info.rcMonitor.left, max_left);
        if SetWindowPos(
            hwnd,
            null_mut(),
            left,
            rect.top,
            0,
            0,
            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        ) == 0
        {
            return Err("SetWindowPos failed".into());
        }
        Ok(wanted != left)
    }
}

fn resume_idle(hwnd: HWND, state: &mut PetState) -> Result<(), String> {
    if state.animation.reset_idle(Instant::now()) {
        render_frame(hwnd, state)?;
    }
    if unsafe { SetTimer(hwnd, ANIMATION_TIMER, state.animation.interval_ms(), None) } == 0 {
        return Err("SetTimer failed".into());
    }
    Ok(())
}

fn play_reaction(hwnd: HWND, state: &mut PetState, reaction: Reaction) -> Result<(), String> {
    state.animation.start_reaction(reaction, Instant::now());
    render_frame(hwnd, state)?;
    if reaction != Reaction::Dragged
        && unsafe { SetTimer(hwnd, ANIMATION_TIMER, state.animation.interval_ms(), None) } == 0
    {
        return Err("SetTimer failed".into());
    }
    Ok(())
}

fn begin_press(hwnd: HWND, state: &mut PetState, double_click: bool) {
    let mut cursor: POINT = unsafe { zeroed() };
    let mut window_rect: RECT = unsafe { zeroed() };
    if unsafe { GetCursorPos(&mut cursor) } == 0
        || unsafe { GetWindowRect(hwnd, &mut window_rect) } == 0
    {
        return;
    }

    state.drag = DragState {
        active: true,
        moved: false,
        double_click,
        start_x: cursor.x,
        start_y: cursor.y,
        offset_x: cursor.x - window_rect.left,
        offset_y: cursor.y - window_rect.top,
        width: window_rect.right - window_rect.left,
        height: window_rect.bottom - window_rect.top,
    };
    if double_click {
        state.click_pending = false;
        unsafe { KillTimer(hwnd, CLICK_TIMER) };
    }
    unsafe {
        KillTimer(hwnd, ANIMATION_TIMER);
        SetCapture(hwnd);
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

unsafe fn pet_state(hwnd: HWND) -> *mut PetState {
    // SAFETY: GWLP_USERDATA is initialized with a PetState pointer immediately after
    // CreateWindowExW succeeds and the Box remains alive for the whole message loop.
    unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut PetState }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_LBUTTONDOWN | WM_LBUTTONDBLCLK => {
            let state = unsafe { pet_state(hwnd) };
            if !state.is_null() {
                // SAFETY: state points to the PetState owned by run() for the lifetime
                // of this window.
                begin_press(hwnd, unsafe { &mut *state }, message == WM_LBUTTONDBLCLK);
            }
            0
        }
        WM_MOUSEMOVE => {
            let state = unsafe { pet_state(hwnd) };
            if !state.is_null() {
                // SAFETY: state points to the PetState owned by run() for the lifetime
                // of this window.
                let state = unsafe { &mut *state };

                if state.drag.active {
                    let mut cursor: POINT = unsafe { zeroed() };
                    if unsafe { GetCursorPos(&mut cursor) } != 0 {
                        if !state.drag.moved
                            && passed_drag_threshold(
                                &state.drag,
                                cursor,
                                unsafe { GetSystemMetrics(SM_CXDRAG) },
                                unsafe { GetSystemMetrics(SM_CYDRAG) },
                            )
                        {
                            state.drag.moved = true;
                            state.click_pending = false;
                            unsafe { KillTimer(hwnd, CLICK_TIMER) };
                            state
                                .animation
                                .start_reaction(Reaction::Dragged, Instant::now());
                            if let Err(error) = render_frame(hwnd, state) {
                                eprintln!("Desktop Pet drag frame failed: {error}");
                            }
                        }
                        if !state.drag.moved {
                            return 0;
                        }
                        let left = cursor.x - state.drag.offset_x;
                        let top = cursor.y - state.drag.offset_y;
                        let mut target_rect = RECT {
                            left,
                            top,
                            right: left + state.drag.width,
                            bottom: top + state.drag.height,
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
            let state = unsafe { pet_state(hwnd) };
            if !state.is_null() {
                let state = unsafe { &mut *state };
                if state.drag.active {
                    let action = release_action(&state.drag);
                    state.drag.active = false;
                    let result = match action {
                        ReleaseAction::Drop => play_reaction(hwnd, state, Reaction::Fall),
                        ReleaseAction::DoubleClick => play_reaction(hwnd, state, Reaction::Special),
                        ReleaseAction::Click => {
                            state.click_pending = true;
                            if unsafe {
                                SetTimer(hwnd, CLICK_TIMER, GetDoubleClickTime().max(1), None)
                            } == 0
                            {
                                state.click_pending = false;
                                play_reaction(hwnd, state, Reaction::Happy)
                            } else {
                                Ok(())
                            }
                        }
                    };
                    if let Err(error) = result {
                        eprintln!("Desktop Pet interaction stopped: {error}");
                    }
                }
            }
            unsafe { ReleaseCapture() };
            0
        }
        WM_CAPTURECHANGED => {
            let state = unsafe { pet_state(hwnd) };
            if !state.is_null() {
                let state = unsafe { &mut *state };
                if state.drag.active {
                    let moved = state.drag.moved;
                    state.drag.active = false;
                    let result = if moved {
                        play_reaction(hwnd, state, Reaction::Fall)
                    } else {
                        resume_idle(hwnd, state)
                    };
                    if let Err(error) = result {
                        eprintln!("Desktop Pet interaction stopped: {error}");
                    }
                }
            }
            0
        }
        WM_TIMER if wparam == CLICK_TIMER => {
            unsafe { KillTimer(hwnd, CLICK_TIMER) };
            let state = unsafe { pet_state(hwnd) };
            if !state.is_null() {
                let state = unsafe { &mut *state };
                if state.click_pending {
                    state.click_pending = false;
                    if !state.drag.active
                        && let Err(error) = play_reaction(hwnd, state, Reaction::Happy)
                    {
                        eprintln!("Desktop Pet click reaction stopped: {error}");
                    }
                }
            }
            0
        }
        WM_TIMER if wparam == ANIMATION_TIMER => {
            let state = unsafe { pet_state(hwnd) };
            if !state.is_null() {
                let state = unsafe { &mut *state };
                if state.drag.active || state.click_pending {
                    return 0;
                }
                let tick = state.animation.tick(Instant::now());
                let mut redraw = tick.redraw;
                let result = (|| -> Result<(), String> {
                    if tick.dx != 0 && move_walk(hwnd, tick.dx)? {
                        state.animation.reverse();
                        redraw = true;
                    }
                    if redraw {
                        render_frame(hwnd, state)?;
                    }
                    if tick.mode_changed
                        && unsafe {
                            SetTimer(hwnd, ANIMATION_TIMER, state.animation.interval_ms(), None)
                        } == 0
                    {
                        return Err("SetTimer failed".into());
                    }
                    Ok(())
                })();
                if let Err(error) = result {
                    unsafe { KillTimer(hwnd, ANIMATION_TIMER) };
                    eprintln!("Desktop Pet animation stopped: {error}");
                }
            }
            0
        }
        WM_DESTROY => {
            unsafe { KillTimer(hwnd, ANIMATION_TIMER) };
            unsafe { KillTimer(hwnd, CLICK_TIMER) };
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
    use image::GenericImageView;
    use windows_sys::Win32::UI::WindowsAndMessaging::DestroyWindow;

    #[test]
    fn click_double_click_and_drag_have_distinct_release_actions() {
        let mut drag = DragState {
            active: true,
            start_x: -100,
            start_y: 50,
            ..DragState::default()
        };
        let cursor = POINT { x: -97, y: 52 };
        assert!(!passed_drag_threshold(&drag, cursor, 4, 4));
        assert_eq!(release_action(&drag), ReleaseAction::Click);

        drag.double_click = true;
        assert_eq!(release_action(&drag), ReleaseAction::DoubleClick);
        assert!(passed_drag_threshold(&drag, POINT { x: -96, y: 52 }, 4, 4));

        drag.moved = true;
        assert_eq!(release_action(&drag), ReleaseAction::Drop);
    }

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

    #[test]
    fn queued_timer_does_not_restart_animation_while_dragging() {
        let pack = CharacterPack::load(
            &std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("assets")
                .join("characters")
                .join("default"),
        )
        .unwrap();
        let image = image::open(&pack.resolve_motion("idle").unwrap().frames[0]).unwrap();
        let (width, height) = image.dimensions();
        let mut state = Box::new(PetState {
            drag: DragState {
                active: true,
                ..DragState::default()
            },
            click_pending: false,
            animation: Animation::new(&pack, Instant::now() - std::time::Duration::from_secs(4))
                .unwrap(),
            scale: 1.0,
            width: width as i32,
            height: height as i32,
        });

        unsafe {
            let hwnd = CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TOOLWINDOW,
                wide_null("STATIC").as_ptr(),
                wide_null("DesktopPet timer test").as_ptr(),
                WS_POPUP,
                200,
                200,
                width as i32,
                height as i32,
                null_mut(),
                null_mut(),
                GetModuleHandleW(null()),
                null_mut(),
            );
            assert!(!hwnd.is_null());
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, (&mut *state as *mut PetState) as isize);
            window_proc(hwnd, WM_TIMER, ANIMATION_TIMER, 0);
            let timer_rearmed = KillTimer(hwnd, ANIMATION_TIMER) != 0;
            let interval = state.animation.interval_ms();
            state.drag.active = false;
            state.click_pending = true;
            window_proc(hwnd, WM_TIMER, ANIMATION_TIMER, 0);
            let timer_rearmed_while_click_pending = KillTimer(hwnd, ANIMATION_TIMER) != 0;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            DestroyWindow(hwnd);
            assert_eq!(interval, 3000);
            assert!(
                !timer_rearmed,
                "queued WM_TIMER rearmed animation during drag"
            );
            assert!(!timer_rearmed_while_click_pending);
        }
    }
}
