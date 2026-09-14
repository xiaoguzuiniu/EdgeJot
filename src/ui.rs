use std::{
    collections::HashMap,
    error::Error,
    ffi::c_void,
    ptr,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicI32, AtomicIsize, AtomicUsize, Ordering},
    },
    time::Instant,
};

use windows::{
    Win32::{
        Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM},
        Graphics::{
            Direct2D::{
                Common::{D2D_RECT_F, D2D1_ALPHA_MODE_IGNORE, D2D1_COLOR_F, D2D1_PIXEL_FORMAT},
                D2D1_DRAW_TEXT_OPTIONS_CLIP, D2D1_FACTORY_TYPE_SINGLE_THREADED,
                D2D1_FEATURE_LEVEL_DEFAULT, D2D1_RENDER_TARGET_PROPERTIES,
                D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1_RENDER_TARGET_USAGE_NONE,
                D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE, D2D1CreateFactory, ID2D1Factory,
            },
            DirectWrite::{
                DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_WEIGHT, DWRITE_MEASURING_MODE_NATURAL,
                DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_PIXEL_GEOMETRY_FLAT,
                DWRITE_RENDERING_MODE_NATURAL_SYMMETRIC, DWRITE_TEXT_ALIGNMENT_LEADING,
                DWRITE_TRIMMING, DWRITE_TRIMMING_GRANULARITY_CHARACTER,
                DWRITE_WORD_WRAPPING_NO_WRAP, DWriteCreateFactory, IDWriteFactory,
                IDWriteRenderingParams,
            },
            Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM,
            Gdi::{
                AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION,
                BeginPaint, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC,
                DeleteObject, EndPaint, GetDC, GetMonitorInfoW, HGDIOBJ, InvalidateRect,
                MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint, PAINTSTRUCT, ReleaseDC,
                SelectObject,
            },
        },
        System::LibraryLoader::GetModuleHandleW,
        UI::{
            HiDpi::{
                DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForSystem, GetDpiForWindow,
                SetProcessDpiAwarenessContext,
            },
            Input::KeyboardAndMouse::{ReleaseCapture, SetCapture},
            WindowsAndMessaging::{
                CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DestroyWindow,
                DispatchMessageW, GetCursorPos, GetMessageW, GetWindowRect, IDC_HAND, KillTimer,
                LoadCursorW, MSG, PostQuitMessage, RegisterClassW, SW_HIDE, SW_SHOWNOACTIVATE,
                SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW, SetCursor, SetTimer,
                SetWindowPos, ShowWindow, TranslateMessage, ULW_ALPHA, UpdateLayeredWindow,
                WINDOW_EX_STYLE, WM_DESTROY, WM_DPICHANGED, WM_ERASEBKGND, WM_LBUTTONDOWN,
                WM_LBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_PAINT, WM_RBUTTONUP, WM_SETCURSOR,
                WM_TIMER, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
                WS_EX_TOPMOST, WS_POPUP,
            },
        },
    },
    core::{Error as WinError, PCWSTR, Result as WinResult},
};

use crate::db::{Database, NoteSummary};

const PILL_WINDOW_WIDTH_DIP: i32 = 22;
const PILL_MIN_HEIGHT_DIP: i32 = 48;
const PILL_MAX_HEIGHT_DIP: i32 = 94;
const PILL_CAPSULE_WIDTH_DIP: i32 = 13;
const PILL_CAPSULE_RIGHT_MARGIN_DIP: i32 = 3;
const PILL_CAPSULE_VERTICAL_INSET_DIP: i32 = 4;
const PILL_DOT_SIZE_DIP: i32 = 5;
const PILL_DOT_SPACING_DIP: i32 = 4;
const PILL_MAX_DOTS: usize = 7;
const LIST_WIDTH_DIP: i32 = 142;
const LIST_VISIBLE_NOTE_LIMIT: usize = 9;
const ANTIALIAS_SAMPLES: i32 = 8;
const CARD_HOVER_SHIFT_DIP: i32 = 9;
const CARD_ANIMATION_DURATION_SECONDS: f32 = 0.15;
const LIST_REVEAL_OFFSET_DIP: i32 = 18;
const LIST_REVEAL_DURATION_SECONDS: f32 = 0.18;
const LIST_SCROLL_ANIMATION_DURATION_SECONDS: f32 = 0.18;
const LIST_SCROLL_SLIDE_DIP: i32 = 8;
const UI_FONT_FACE: &str = "Microsoft YaHei UI";
const SYMBOL_FONT_FACE: &str = "Segoe UI";
const DECK_LABEL_FONT_DIP: i32 = 10;
const CARD_LABEL_FONT_DIP: i32 = 12;
const NORMAL_FONT_WEIGHT: i32 = 400;
const MEDIUM_FONT_WEIGHT: i32 = 500;

static DATABASE: OnceLock<Mutex<Database>> = OnceLock::new();
static NOTES: OnceLock<Mutex<Vec<NoteSummary>>> = OnceLock::new();
static CARD_HOVER_PROGRESS: OnceLock<Mutex<Vec<f32>>> = OnceLock::new();
static LAST_ANIMATION_TICK: OnceLock<Mutex<Instant>> = OnceLock::new();
static LIST_REVEAL_ANIMATION: OnceLock<Mutex<ListRevealAnimation>> = OnceLock::new();
static LIST_RENDER_CACHE: OnceLock<Mutex<Option<ListRenderCache>>> = OnceLock::new();
static CARD_SPRITE_CACHE: OnceLock<Mutex<HashMap<CardSpriteKey, Arc<Vec<CardSprite>>>>> =
    OnceLock::new();
static LIST_SCROLL_ANIMATION: OnceLock<Mutex<ListScrollAnimation>> = OnceLock::new();
static PILL_WINDOW: AtomicIsize = AtomicIsize::new(0);
static LIST_WINDOW: AtomicIsize = AtomicIsize::new(0);
static LIST_SCROLL_OFFSET: AtomicUsize = AtomicUsize::new(0);
static DRAGGING: AtomicBool = AtomicBool::new(false);
static DRAG_OFFSET_Y: AtomicI32 = AtomicI32::new(0);

#[derive(Clone, Copy)]
struct Color {
    red: u8,
    green: u8,
    blue: u8,
}

struct TextStyle<'a> {
    face: &'a str,
    pixel_height: i32,
    weight: i32,
    color: Color,
}

struct TextRasterizer {
    d2d_factory: ID2D1Factory,
    write_factory: IDWriteFactory,
    rendering_params: IDWriteRenderingParams,
}

impl TextRasterizer {
    unsafe fn new() -> WinResult<Self> {
        // SAFETY: Factories are created and used only on the UI thread.
        unsafe {
            let d2d_factory = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
            let write_factory: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
            let rendering_params = write_factory.CreateCustomRenderingParams(
                2.2,
                1.0,
                0.0,
                DWRITE_PIXEL_GEOMETRY_FLAT,
                DWRITE_RENDERING_MODE_NATURAL_SYMMETRIC,
            )?;
            Ok(Self {
                d2d_factory,
                write_factory,
                rendering_params,
            })
        }
    }
}

struct CardSprite {
    width: i32,
    height: i32,
    pixels: Vec<u8>,
}

#[derive(Clone, Hash, PartialEq, Eq)]
struct CardSpriteKey {
    title: String,
    color: String,
    resting_width: i32,
    card_height: i32,
    dpi: u32,
}

struct ListRenderCache {
    width: i32,
    height: i32,
    dpi: u32,
    total_note_count: usize,
    note_keys: Vec<(String, String)>,
    base_pixels: Vec<u8>,
    card_sprites: Vec<Arc<Vec<CardSprite>>>,
}

struct ListRevealAnimation {
    progress: f32,
    target: f32,
    last_tick: Instant,
    final_x: i32,
    final_y: i32,
    width: i32,
    height: i32,
    dpi: u32,
}

struct ListScrollAnimation {
    progress: f32,
    direction: i32,
    highlighted_card: Option<usize>,
    last_tick: Instant,
    active: bool,
}

#[derive(Clone, Copy)]
struct ListScrollVisual {
    offset_y: f32,
    highlighted_card: Option<usize>,
    card_shift: f32,
}

#[derive(Clone, Copy)]
struct ListRevealFrame {
    progress: f32,
    target: f32,
    final_x: i32,
    final_y: i32,
    width: i32,
    height: i32,
    dpi: u32,
    changed: bool,
}

impl ListRenderCache {
    fn matches(
        &self,
        width: i32,
        height: i32,
        dpi: u32,
        notes: &[NoteSummary],
        total_note_count: usize,
    ) -> bool {
        self.width == width
            && self.height == height
            && self.dpi == dpi
            && self.total_note_count == total_note_count
            && self.note_keys.len() == notes.len()
            && self
                .note_keys
                .iter()
                .zip(notes)
                .all(|((title, color), note)| title == &note.title && color == &note.color)
    }
}

pub fn run(database: Database) -> Result<(), Box<dyn Error>> {
    let notes = database.list_notes()?;
    let note_count = notes.len();
    DATABASE
        .set(Mutex::new(database))
        .map_err(|_| std::io::Error::other("数据库状态已初始化"))?;
    NOTES
        .set(Mutex::new(notes))
        .map_err(|_| std::io::Error::other("便签状态已初始化"))?;
    CARD_HOVER_PROGRESS
        .set(Mutex::new(vec![0.0; note_count]))
        .map_err(|_| std::io::Error::other("标签动画状态已初始化"))?;
    LAST_ANIMATION_TICK
        .set(Mutex::new(Instant::now()))
        .map_err(|_| std::io::Error::other("动画时钟已初始化"))?;
    LIST_REVEAL_ANIMATION
        .set(Mutex::new(ListRevealAnimation {
            progress: 0.0,
            target: 0.0,
            last_tick: Instant::now(),
            final_x: 0,
            final_y: 0,
            width: 0,
            height: 0,
            dpi: 96,
        }))
        .map_err(|_| std::io::Error::other("列表展开动画状态已初始化"))?;
    LIST_RENDER_CACHE
        .set(Mutex::new(None))
        .map_err(|_| std::io::Error::other("绘制缓存已初始化"))?;
    CARD_SPRITE_CACHE
        .set(Mutex::new(HashMap::new()))
        .map_err(|_| std::io::Error::other("卡片缓存已初始化"))?;
    LIST_SCROLL_ANIMATION
        .set(Mutex::new(ListScrollAnimation {
            progress: 1.0,
            direction: 0,
            highlighted_card: None,
            last_tick: Instant::now(),
            active: false,
        }))
        .map_err(|_| std::io::Error::other("列表滚动动画状态已初始化"))?;

    // SAFETY: DPI awareness is configured before creating windows. All HWNDs
    // and GDI resources are used on this UI thread.
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)?;
        let instance = HINSTANCE(GetModuleHandleW(None)?.0);

        let pill_class = utf16("NotyFloatingPill");
        let list_class = utf16("NotyNoteList");
        register_window_class(
            instance,
            PCWSTR(pill_class.as_ptr()),
            pill_window_proc,
            IDC_HAND,
        )?;
        register_window_class(
            instance,
            PCWSTR(list_class.as_ptr()),
            list_window_proc,
            IDC_HAND,
        )?;

        let dpi = GetDpiForSystem();
        let pill_width = scale_for_dpi(PILL_WINDOW_WIDTH_DIP, dpi);
        let pill_height = pill_height(note_count, dpi);
        let work_area = work_area_at(POINT { x: 0, y: 0 })?;
        let x = work_area.right - pill_width;
        let y = work_area.top + (work_area.bottom - work_area.top - pill_height) / 2;

        let pill_title = utf16("Noty");
        let pill_window = CreateWindowExW(
            WINDOW_EX_STYLE(
                WS_EX_TOPMOST.0 | WS_EX_TOOLWINDOW.0 | WS_EX_NOACTIVATE.0 | WS_EX_LAYERED.0,
            ),
            PCWSTR(pill_class.as_ptr()),
            PCWSTR(pill_title.as_ptr()),
            WS_POPUP,
            x,
            y,
            pill_width,
            pill_height,
            None,
            None,
            Some(instance),
            None,
        )?;
        PILL_WINDOW.store(pill_window.0 as isize, Ordering::Release);

        let notes = current_notes();
        let visible_notes = note_page(&notes);
        let list_height = list_height(visible_notes.len(), dpi);
        let list_title = utf16("Noty 便签列表");
        let list_window = CreateWindowExW(
            WINDOW_EX_STYLE(
                WS_EX_TOPMOST.0 | WS_EX_TOOLWINDOW.0 | WS_EX_NOACTIVATE.0 | WS_EX_LAYERED.0,
            ),
            PCWSTR(list_class.as_ptr()),
            PCWSTR(list_title.as_ptr()),
            WS_POPUP,
            work_area.right - scale_for_dpi(LIST_WIDTH_DIP, dpi),
            y,
            scale_for_dpi(LIST_WIDTH_DIP, dpi),
            list_height,
            None,
            None,
            Some(instance),
            None,
        )?;
        LIST_WINDOW.store(list_window.0 as isize, Ordering::Release);

        render_note_list(
            list_window,
            scale_for_dpi(LIST_WIDTH_DIP, dpi),
            list_height,
            dpi,
            &visible_notes,
            notes.len(),
            255,
        )?;
        render_pill(pill_window, pill_width, pill_height, dpi, &notes)?;
        let _ = ShowWindow(pill_window, SW_SHOWNOACTIVATE);

        let mut message = MSG::default();
        loop {
            let status = GetMessageW(&mut message, None, 0, 0);
            if status.0 == -1 {
                return Err(Box::new(WinError::from_thread()));
            }
            if !status.as_bool() {
                break;
            }
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }

    Ok(())
}

unsafe fn register_window_class(
    instance: HINSTANCE,
    class_name: PCWSTR,
    window_proc: unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT,
    cursor: PCWSTR,
) -> WinResult<()> {
    // SAFETY: The class name remains alive for the entire message loop.
    unsafe {
        let window_class = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            hCursor: LoadCursorW(None, cursor)?,
            lpszClassName: class_name,
            ..Default::default()
        };
        if RegisterClassW(&window_class) == 0 {
            return Err(WinError::from_thread());
        }
        Ok(())
    }
}

unsafe extern "system" fn pill_window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // SAFETY: Windows calls this procedure on the thread that owns `window`.
    unsafe {
        match message {
            WM_ERASEBKGND => LRESULT(1),
            WM_SETCURSOR => set_hand_cursor(),
            WM_MOUSEMOVE if !DRAGGING.load(Ordering::Relaxed) => {
                show_note_list(window);
                LRESULT(0)
            }
            WM_LBUTTONDOWN => {
                let mut cursor = POINT::default();
                let mut bounds = RECT::default();
                if GetCursorPos(&mut cursor).is_ok() && GetWindowRect(window, &mut bounds).is_ok() {
                    DRAG_OFFSET_Y.store(cursor.y - bounds.top, Ordering::Relaxed);
                    DRAGGING.store(true, Ordering::Relaxed);
                    let _ = SetCapture(window);
                }
                LRESULT(0)
            }
            WM_MOUSEMOVE => {
                move_along_edge(window);
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                DRAGGING.store(false, Ordering::Relaxed);
                let _ = ReleaseCapture();
                LRESULT(0)
            }
            WM_DPICHANGED => {
                resize_pill_for_dpi(window, wparam, lparam);
                LRESULT(0)
            }
            WM_RBUTTONUP => {
                close_app();
                LRESULT(0)
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(window, message, wparam, lparam),
        }
    }
}

unsafe extern "system" fn list_window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // SAFETY: Windows calls this procedure on the thread that owns `window`.
    unsafe {
        match message {
            WM_ERASEBKGND => LRESULT(1),
            WM_SETCURSOR => set_hand_cursor(),
            WM_PAINT => {
                refresh_note_list(window);
                LRESULT(0)
            }
            WM_MOUSEMOVE => LRESULT(0),
            WM_TIMER => {
                update_note_list_interaction(window);
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                handle_list_click(window, lparam);
                LRESULT(0)
            }
            WM_MOUSEWHEEL => {
                scroll_note_list(window, wparam);
                LRESULT(0)
            }
            WM_DPICHANGED => {
                let _ = InvalidateRect(Some(window), None, true);
                LRESULT(0)
            }
            WM_RBUTTONUP => {
                close_app();
                LRESULT(0)
            }
            WM_DESTROY => LRESULT(0),
            _ => DefWindowProcW(window, message, wparam, lparam),
        }
    }
}

unsafe fn show_note_list(pill_window: HWND) {
    // SAFETY: Both windows belong to the current UI thread.
    unsafe {
        let list_window = load_window(&LIST_WINDOW);
        if list_window.0.is_null() {
            return;
        }
        let mut pill_bounds = RECT::default();
        if GetWindowRect(pill_window, &mut pill_bounds).is_err() {
            return;
        }
        let center = POINT {
            x: (pill_bounds.left + pill_bounds.right) / 2,
            y: (pill_bounds.top + pill_bounds.bottom) / 2,
        };
        let Ok(work_area) = work_area_at(center) else {
            return;
        };
        let dpi = GetDpiForWindow(pill_window);
        let width = scale_for_dpi(LIST_WIDTH_DIP, dpi);
        let notes = current_notes();
        let visible_notes = note_page(&notes);
        let height = list_height(visible_notes.len(), dpi);
        let final_x = work_area.right - width;
        let final_y = (center.y - height / 2).clamp(work_area.top, work_area.bottom - height);
        let initial_x = final_x + scale_for_dpi(LIST_REVEAL_OFFSET_DIP, dpi);

        begin_list_reveal(final_x, final_y, width, height, dpi);

        let _ = SetWindowPos(
            list_window,
            None,
            initial_x,
            final_y,
            width,
            height,
            SWP_NOACTIVATE | SWP_NOZORDER | SWP_SHOWWINDOW,
        );
        let _ = render_note_list(
            list_window,
            width,
            height,
            dpi,
            &visible_notes,
            notes.len(),
            0,
        );
        reset_animation_clock();
        let _ = SetTimer(Some(list_window), 1, 16, None);
        let _ = ShowWindow(pill_window, SW_HIDE);
    }
}

unsafe fn hide_note_list(list_window: HWND) {
    set_list_reveal_target(0.0);
    // SAFETY: The timer belongs to this list window and drives the closing frames.
    unsafe {
        let _ = SetTimer(Some(list_window), 1, 16, None);
    }
}

unsafe fn finish_hiding_note_list(list_window: HWND) {
    // SAFETY: Both windows belong to the current UI thread.
    unsafe {
        let _ = KillTimer(Some(list_window), 1);
        let _ = ShowWindow(list_window, SW_HIDE);
        reset_card_hover_progress();
        reset_animation_clock();
        let pill_window = load_window(&PILL_WINDOW);
        if !pill_window.0.is_null() {
            let _ = ShowWindow(pill_window, SW_SHOWNOACTIVATE);
        }
    }
}

unsafe fn update_note_list_interaction(window: HWND) {
    // SAFETY: The timer runs on the window's UI thread and only reads its bounds.
    unsafe {
        let mut cursor = POINT::default();
        let mut bounds = RECT::default();
        if GetCursorPos(&mut cursor).is_err() || GetWindowRect(window, &mut bounds).is_err() {
            return;
        }
        let cursor_inside = cursor.x >= bounds.left
            && cursor.x < bounds.right
            && cursor.y >= bounds.top
            && cursor.y < bounds.bottom;
        if !cursor_inside {
            hide_note_list(window);
        }

        let Some(frame) = advance_list_reveal() else {
            return;
        };
        let scroll_changed = advance_list_scroll_animation();
        if frame.target == 0.0 && frame.progress <= f32::EPSILON {
            finish_hiding_note_list(window);
            return;
        }

        let eased_reveal = ease_out_cubic(frame.progress);
        let x = frame.final_x
            + ((1.0 - eased_reveal) * scale_for_dpi(LIST_REVEAL_OFFSET_DIP, frame.dpi) as f32)
                .round() as i32;
        let _ = SetWindowPos(
            window,
            None,
            x,
            frame.final_y,
            frame.width,
            frame.height,
            SWP_NOACTIVATE | SWP_NOZORDER,
        );

        let all_notes = current_notes();
        let notes = note_page(&all_notes);
        let hovered = if !cursor_inside || frame.target == 0.0 {
            None
        } else {
            let local_x = cursor.x - x;
            let local_y = cursor.y - frame.final_y;
            let progress = card_hover_progress(notes.len());
            (0..notes.len()).rev().find(|index| {
                let top = scale_for_dpi(39 + *index as i32 * 48, frame.dpi);
                let left = card_left(*index, frame.dpi, &progress);
                local_x >= left
                    && local_x < frame.width
                    && local_y >= top
                    && local_y < top + scale_for_dpi(65, frame.dpi)
            })
        };
        let cards_changed = advance_card_hover_progress(notes.len(), hovered);
        if frame.changed || cards_changed || scroll_changed {
            let alpha = (eased_reveal * 255.0).round() as u8;
            let _ = render_note_list(
                window,
                frame.width,
                frame.height,
                frame.dpi,
                &notes,
                all_notes.len(),
                alpha,
            );
        }
    }
}

unsafe fn handle_list_click(window: HWND, lparam: LPARAM) {
    let x = lparam.0 as i16 as i32;
    let y = (lparam.0 >> 16) as i16 as i32;
    // SAFETY: The window is valid and owned by this UI thread.
    unsafe {
        let dpi = GetDpiForWindow(window);
        let footer_top = footer_y(note_page(&current_notes()).len(), dpi);
        if x >= scale_for_dpi(15, dpi)
            && x < scale_for_dpi(73, dpi)
            && y >= footer_top
            && y < footer_top + scale_for_dpi(40, dpi)
        {
            add_note();
        }
    }
}

unsafe fn scroll_note_list(window: HWND, wparam: WPARAM) {
    let delta = ((wparam.0 >> 16) & 0xffff) as u16 as i16 as i32;
    if delta == 0 {
        return;
    }

    let all_notes = current_notes();
    let max_offset = max_scroll_offset(all_notes.len());
    if max_offset == 0 {
        return;
    }
    let current = LIST_SCROLL_OFFSET.load(Ordering::Relaxed).min(max_offset);
    let next = scroll_offset_after_delta(all_notes.len(), current, delta);
    if next == current {
        return;
    }

    LIST_SCROLL_OFFSET.store(next, Ordering::Relaxed);
    reset_card_hover_progress();
    reset_animation_clock();
    let notes = note_page(&all_notes);
    begin_list_scroll_animation(delta, notes.len());

    // SAFETY: The wheel message and redraw execute on the list window's UI thread.
    unsafe {
        let mut bounds = RECT::default();
        if GetWindowRect(window, &mut bounds).is_ok() {
            let _ = render_note_list(
                window,
                bounds.right - bounds.left,
                bounds.bottom - bounds.top,
                GetDpiForWindow(window),
                &notes,
                all_notes.len(),
                list_reveal_alpha(),
            );
        }
    }
}

fn add_note() {
    let Some(database) = DATABASE.get() else {
        return;
    };
    let Ok(database) = database.lock() else {
        return;
    };
    if database.create_note().is_err() {
        return;
    }
    let Ok(updated_notes) = database.list_notes() else {
        return;
    };
    drop(database);
    LIST_SCROLL_OFFSET.store(0, Ordering::Relaxed);
    reset_card_hover_progress();

    if let Some(notes) = NOTES.get()
        && let Ok(mut notes) = notes.lock()
    {
        *notes = updated_notes.clone();
    }

    // SAFETY: Refresh calls execute from the UI thread's click handler.
    unsafe {
        let pill_window = load_window(&PILL_WINDOW);
        if !pill_window.0.is_null() {
            let mut bounds = RECT::default();
            if GetWindowRect(pill_window, &mut bounds).is_ok() {
                let dpi = GetDpiForWindow(pill_window);
                let width = scale_for_dpi(PILL_WINDOW_WIDTH_DIP, dpi);
                let height = pill_height(updated_notes.len(), dpi);
                let center = POINT {
                    x: (bounds.left + bounds.right) / 2,
                    y: (bounds.top + bounds.bottom) / 2,
                };
                if let Ok(work_area) = work_area_at(center) {
                    let x = work_area.right - width;
                    let y = (center.y - height / 2).clamp(work_area.top, work_area.bottom - height);
                    let _ = SetWindowPos(
                        pill_window,
                        None,
                        x,
                        y,
                        width,
                        height,
                        SWP_NOACTIVATE | SWP_NOZORDER,
                    );
                    let _ = render_pill(pill_window, width, height, dpi, &updated_notes);
                }
            }
        }
        let list_window = load_window(&LIST_WINDOW);
        if !list_window.0.is_null() {
            let mut bounds = RECT::default();
            if GetWindowRect(list_window, &mut bounds).is_ok() {
                let dpi = GetDpiForWindow(list_window);
                let width = scale_for_dpi(LIST_WIDTH_DIP, dpi);
                let visible_notes = note_page(&updated_notes);
                let height = list_height(visible_notes.len(), dpi);
                let pill_window = load_window(&PILL_WINDOW);
                let mut pill_bounds = RECT::default();
                if pill_window.0.is_null() || GetWindowRect(pill_window, &mut pill_bounds).is_err()
                {
                    return;
                }
                let center = POINT {
                    x: (pill_bounds.left + pill_bounds.right) / 2,
                    y: (pill_bounds.top + pill_bounds.bottom) / 2,
                };
                if let Ok(work_area) = work_area_at(center) {
                    let x = work_area.right - width;
                    let y = (center.y - height / 2).clamp(work_area.top, work_area.bottom - height);
                    let _ = SetWindowPos(
                        list_window,
                        None,
                        x,
                        y,
                        width,
                        height,
                        SWP_NOACTIVATE | SWP_NOZORDER,
                    );
                    update_list_reveal_geometry(x, y, width, height, dpi);
                }
            }
            let dpi = GetDpiForWindow(list_window);
            let mut bounds = RECT::default();
            if GetWindowRect(list_window, &mut bounds).is_ok() {
                let visible_notes = note_page(&updated_notes);
                let _ = render_note_list(
                    list_window,
                    bounds.right - bounds.left,
                    bounds.bottom - bounds.top,
                    dpi,
                    &visible_notes,
                    updated_notes.len(),
                    list_reveal_alpha(),
                );
            }
        }
    }
}

unsafe fn refresh_note_list(window: HWND) {
    // SAFETY: BeginPaint/EndPaint only validate this window's update region.
    unsafe {
        let mut paint = PAINTSTRUCT::default();
        let _ = BeginPaint(window, &mut paint);
        let _ = EndPaint(window, &paint);

        let mut bounds = RECT::default();
        if GetWindowRect(window, &mut bounds).is_ok() {
            let notes = current_notes();
            let visible_notes = note_page(&notes);
            let _ = render_note_list(
                window,
                bounds.right - bounds.left,
                bounds.bottom - bounds.top,
                GetDpiForWindow(window),
                &visible_notes,
                notes.len(),
                list_reveal_alpha(),
            );
        }
    }
}

unsafe fn render_note_list(
    window: HWND,
    width: i32,
    height: i32,
    dpi: u32,
    notes: &[NoteSummary],
    total_note_count: usize,
    alpha: u8,
) -> WinResult<()> {
    let progress = card_hover_progress(notes.len());
    let Some(cache_storage) = LIST_RENDER_CACHE.get() else {
        return Err(WinError::from_thread());
    };
    let mut cache_storage = cache_storage.lock().map_err(|_| WinError::from_thread())?;
    let needs_rebuild = cache_storage
        .as_ref()
        .is_none_or(|cache| !cache.matches(width, height, dpi, notes, total_note_count));
    if needs_rebuild {
        // SAFETY: Cache construction uses temporary GDI resources on this UI thread.
        *cache_storage =
            Some(unsafe { build_list_render_cache(width, height, dpi, notes, total_note_count)? });
    }

    let cache = cache_storage
        .as_ref()
        .expect("render cache was just created");
    let mut pixels = cache.base_pixels.clone();
    let scroll_visual = list_scroll_visual(dpi);
    for (index, variants) in cache.card_sprites.iter().enumerate() {
        let top = (scale_for_dpi(39 + index as i32 * 48, dpi) as f32 + scroll_visual.offset_y)
            .round() as i32;
        let mut shift = card_shift_f32(index, dpi, &progress);
        if scroll_visual.highlighted_card == Some(index) {
            shift = shift.max(scroll_visual.card_shift);
        }
        composite_interpolated_card(
            &mut pixels,
            SIZE {
                cx: width,
                cy: height,
            },
            variants,
            shift,
            top,
            scale_for_dpi(39, dpi),
            footer_y(notes.len(), dpi),
        );
    }
    drop(cache_storage);

    // SAFETY: `pixels` is a premultiplied BGRA buffer sized for this window.
    unsafe { present_layered_pixels(window, width, height, &pixels, alpha) }
}

unsafe fn build_list_render_cache(
    width: i32,
    height: i32,
    dpi: u32,
    notes: &[NoteSummary],
    total_note_count: usize,
) -> WinResult<ListRenderCache> {
    let text_rasterizer = unsafe { TextRasterizer::new()? };
    let mut base_pixels = render_list_base_shapes(width, height, dpi, notes.len());
    // SAFETY: Text is rasterized once into cached premultiplied buffers.
    unsafe {
        draw_text_layer(
            &text_rasterizer,
            &mut base_pixels,
            width,
            height,
            "随 手 记",
            RECT {
                left: scale_for_dpi(15, dpi),
                top: scale_for_dpi(7, dpi),
                right: scale_for_dpi(88, dpi),
                bottom: scale_for_dpi(34, dpi),
            },
            TextStyle {
                face: UI_FONT_FACE,
                pixel_height: scale_for_dpi(DECK_LABEL_FONT_DIP, dpi),
                weight: NORMAL_FONT_WEIGHT,
                color: Color::new(115, 108, 96),
            },
        )?;
        draw_text_layer(
            &text_rasterizer,
            &mut base_pixels,
            width,
            height,
            &format!("{total_note_count:02}"),
            RECT {
                left: scale_for_dpi(112, dpi),
                top: scale_for_dpi(7, dpi),
                right: scale_for_dpi(133, dpi),
                bottom: scale_for_dpi(34, dpi),
            },
            TextStyle {
                face: UI_FONT_FACE,
                pixel_height: scale_for_dpi(11, dpi),
                weight: NORMAL_FONT_WEIGHT,
                color: Color::new(141, 132, 117),
            },
        )?;

        let footer_top = footer_y(notes.len(), dpi);
        for (text, left, right) in [("+", 29, 62), ("›", 83, 116)] {
            draw_text_layer(
                &text_rasterizer,
                &mut base_pixels,
                width,
                height,
                text,
                RECT {
                    left: scale_for_dpi(left, dpi),
                    top: footer_top - scale_for_dpi(if text == "+" { 1 } else { 2 }, dpi),
                    right: scale_for_dpi(right, dpi),
                    bottom: footer_top + scale_for_dpi(40, dpi),
                },
                TextStyle {
                    face: SYMBOL_FONT_FACE,
                    pixel_height: scale_for_dpi(25, dpi),
                    weight: NORMAL_FONT_WEIGHT,
                    color: Color::new(73, 70, 56),
                },
            )?;
        }
    }

    let max_shift = scale_for_dpi(CARD_HOVER_SHIFT_DIP, dpi);
    let card_height = scale_for_dpi(65, dpi);
    let resting_width = width - scale_for_dpi(17, dpi);
    let mut card_sprites = Vec::with_capacity(notes.len());
    for note in notes {
        let key = CardSpriteKey {
            title: note.title.clone(),
            color: note.color.clone(),
            resting_width,
            card_height,
            dpi,
        };
        let cached = CARD_SPRITE_CACHE
            .get()
            .and_then(|cache| cache.lock().ok()?.get(&key).cloned());
        if let Some(cached) = cached {
            card_sprites.push(cached);
            continue;
        }

        let mut variants = Vec::with_capacity((max_shift + 1) as usize);
        for shift in 0..=max_shift {
            let sprite_width = resting_width + shift;
            let mut sprite_pixels = vec![0_u8; (sprite_width * card_height * 4) as usize];
            let card = RECT {
                left: 0,
                top: 0,
                right: sprite_width,
                bottom: card_height,
            };
            let (fill, border) = card_colors(&note.color);
            paint_rounded_rect(
                &mut sprite_pixels,
                sprite_width,
                card_height,
                card,
                scale_for_dpi(12, dpi) as f32,
                border,
            );
            paint_rounded_rect(
                &mut sprite_pixels,
                sprite_width,
                card_height,
                inset_rect(card, scale_for_dpi(1, dpi)),
                scale_for_dpi(11, dpi) as f32,
                fill,
            );
            // SAFETY: The title is drawn into this sprite before it enters the cache.
            unsafe {
                draw_text_layer(
                    &text_rasterizer,
                    &mut sprite_pixels,
                    sprite_width,
                    card_height,
                    &note.title,
                    RECT {
                        left: scale_for_dpi(14, dpi),
                        top: scale_for_dpi(6, dpi),
                        right: sprite_width - scale_for_dpi(13, dpi),
                        bottom: scale_for_dpi(38, dpi),
                    },
                    TextStyle {
                        face: UI_FONT_FACE,
                        pixel_height: scale_for_dpi(CARD_LABEL_FONT_DIP, dpi),
                        weight: MEDIUM_FONT_WEIGHT,
                        color: Color::new(75, 71, 59),
                    },
                )?;
            }
            variants.push(CardSprite {
                width: sprite_width,
                height: card_height,
                pixels: sprite_pixels,
            });
        }
        let variants = Arc::new(variants);
        if let Some(cache) = CARD_SPRITE_CACHE.get()
            && let Ok(mut cache) = cache.lock()
        {
            cache.insert(key, Arc::clone(&variants));
        }
        card_sprites.push(variants);
    }

    Ok(ListRenderCache {
        width,
        height,
        dpi,
        total_note_count,
        note_keys: notes
            .iter()
            .map(|note| (note.title.clone(), note.color.clone()))
            .collect(),
        base_pixels,
        card_sprites,
    })
}

fn render_list_base_shapes(width: i32, height: i32, dpi: u32, note_count: usize) -> Vec<u8> {
    let mut pixels = vec![0_u8; (width * height * 4) as usize];
    let footer_top = footer_y(note_count, dpi);
    let footer = RECT {
        left: scale_for_dpi(18, dpi),
        top: footer_top,
        right: width - scale_for_dpi(6, dpi),
        bottom: footer_top + scale_for_dpi(40, dpi),
    };
    paint_rounded_rect(
        &mut pixels,
        width,
        height,
        footer,
        scale_for_dpi(13, dpi) as f32,
        Color::new(221, 214, 202),
    );
    paint_rounded_rect(
        &mut pixels,
        width,
        height,
        inset_rect(footer, scale_for_dpi(1, dpi)),
        scale_for_dpi(12, dpi) as f32,
        Color::new(244, 240, 233),
    );
    paint_rect(
        &mut pixels,
        width,
        height,
        RECT {
            left: scale_for_dpi(72, dpi),
            top: footer.top + scale_for_dpi(11, dpi),
            right: scale_for_dpi(73, dpi),
            bottom: footer.bottom - scale_for_dpi(11, dpi),
        },
        Color::new(217, 209, 196),
    );
    pixels
}

fn composite_interpolated_card(
    destination: &mut [u8],
    destination_size: SIZE,
    variants: &[CardSprite],
    shift: f32,
    destination_y: i32,
    clip_top: i32,
    clip_bottom: i32,
) {
    let lower = &variants[(shift.floor() as usize).min(variants.len() - 1)];
    let upper = &variants[(shift.ceil() as usize).min(variants.len() - 1)];
    let fraction = shift.fract();
    let left = destination_size.cx - lower.width.max(upper.width);

    for source_y in 0..lower.height.max(upper.height) {
        let target_y = destination_y + source_y;
        if target_y < clip_top || target_y >= clip_bottom || target_y >= destination_size.cy {
            continue;
        }
        for target_x in left.max(0)..destination_size.cx {
            let target_offset = ((target_y * destination_size.cx + target_x) * 4) as usize;
            let lower_x = target_x - (destination_size.cx - lower.width);
            let upper_x = target_x - (destination_size.cx - upper.width);
            let mut interpolated = [0_u8; 4];
            for (channel, value) in interpolated.iter_mut().enumerate() {
                let from = sprite_channel(lower, lower_x, source_y, channel) as f32;
                let to = sprite_channel(upper, upper_x, source_y, channel) as f32;
                *value = (from + (to - from) * fraction).round() as u8;
            }
            let alpha = interpolated[3] as u32;
            let inverse = 255 - alpha;
            for channel in 0..3 {
                destination[target_offset + channel] = (interpolated[channel] as u32
                    + destination[target_offset + channel] as u32 * inverse / 255)
                    .min(255) as u8;
            }
            destination[target_offset + 3] =
                (alpha + destination[target_offset + 3] as u32 * inverse / 255).min(255) as u8;
        }
    }
}

fn sprite_channel(sprite: &CardSprite, x: i32, y: i32, channel: usize) -> u8 {
    if x < 0 || x >= sprite.width || y < 0 || y >= sprite.height {
        return 0;
    }
    sprite.pixels[((y * sprite.width + x) * 4) as usize + channel]
}

fn paint_rounded_rect(
    pixels: &mut [u8],
    width: i32,
    height: i32,
    rect: RECT,
    radius: f32,
    color: Color,
) {
    let samples = 4_i32;
    let sample_count = (samples * samples) as u32;
    for y in rect.top.max(0)..rect.bottom.min(height) {
        for x in rect.left.max(0)..rect.right.min(width) {
            let mut covered = 0_u32;
            for sy in 0..samples {
                for sx in 0..samples {
                    let px = x as f32 + (sx as f32 + 0.5) / samples as f32;
                    let py = y as f32 + (sy as f32 + 0.5) / samples as f32;
                    if inside_rounded_rect(px, py, rect, radius) {
                        covered += 1;
                    }
                }
            }
            composite_pixel(
                pixels,
                width,
                x,
                y,
                color,
                ((covered * 255) / sample_count) as u8,
            );
        }
    }
}

fn paint_rect(pixels: &mut [u8], width: i32, height: i32, rect: RECT, color: Color) {
    for y in rect.top.max(0)..rect.bottom.min(height) {
        for x in rect.left.max(0)..rect.right.min(width) {
            composite_pixel(pixels, width, x, y, color, 255);
        }
    }
}

fn inside_rounded_rect(x: f32, y: f32, rect: RECT, radius: f32) -> bool {
    let left = rect.left as f32;
    let right = rect.right as f32;
    let top = rect.top as f32;
    let bottom = rect.bottom as f32;
    let center_x = x.clamp(left + radius, right - radius);
    let center_y = y.clamp(top + radius, bottom - radius);
    let dx = x - center_x;
    let dy = y - center_y;
    dx * dx + dy * dy <= radius * radius
}

fn inset_rect(rect: RECT, inset: i32) -> RECT {
    RECT {
        left: rect.left + inset,
        top: rect.top + inset,
        right: rect.right - inset,
        bottom: rect.bottom - inset,
    }
}

fn composite_pixel(pixels: &mut [u8], width: i32, x: i32, y: i32, color: Color, alpha: u8) {
    if alpha == 0 {
        return;
    }
    let offset = ((y * width + x) * 4) as usize;
    let source_alpha = alpha as u32;
    let inverse = 255 - source_alpha;
    pixels[offset] =
        ((color.blue as u32 * source_alpha + pixels[offset] as u32 * inverse) / 255) as u8;
    pixels[offset + 1] =
        ((color.green as u32 * source_alpha + pixels[offset + 1] as u32 * inverse) / 255) as u8;
    pixels[offset + 2] =
        ((color.red as u32 * source_alpha + pixels[offset + 2] as u32 * inverse) / 255) as u8;
    pixels[offset + 3] = (source_alpha + pixels[offset + 3] as u32 * inverse / 255).min(255) as u8;
}

unsafe fn draw_text_layer(
    renderer: &TextRasterizer,
    pixels: &mut [u8],
    width: i32,
    height: i32,
    text: &str,
    rect: RECT,
    style: TextStyle<'_>,
) -> WinResult<()> {
    // SAFETY: DirectWrite rasterizes into a temporary D2D DC target. The resulting
    // grayscale coverage is composited into the premultiplied layered-window buffer.
    unsafe {
        let face = utf16(style.face);
        let locale = utf16("zh-CN");
        let text_format = renderer.write_factory.CreateTextFormat(
            PCWSTR(face.as_ptr()),
            None,
            DWRITE_FONT_WEIGHT(style.weight),
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            style.pixel_height as f32,
            PCWSTR(locale.as_ptr()),
        )?;
        text_format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_LEADING)?;
        text_format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
        text_format.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
        let trimming = DWRITE_TRIMMING {
            granularity: DWRITE_TRIMMING_GRANULARITY_CHARACTER,
            delimiter: 0,
            delimiterCount: 0,
        };
        let ellipsis = renderer
            .write_factory
            .CreateEllipsisTrimmingSign(&text_format)?;
        text_format.SetTrimming(&trimming, &ellipsis)?;

        let screen_dc = GetDC(None);
        let memory_dc = CreateCompatibleDC(Some(screen_dc));
        let bitmap_info = top_down_bitmap_info(width, height);
        let mut bits: *mut c_void = ptr::null_mut();
        let bitmap = CreateDIBSection(
            Some(screen_dc),
            &bitmap_info,
            DIB_RGB_COLORS,
            &mut bits,
            None,
            0,
        )?;
        ptr::write_bytes(bits.cast::<u8>(), 0, (width * height * 4) as usize);
        let previous_bitmap = SelectObject(memory_dc, HGDIOBJ(bitmap.0));
        let render_properties = D2D1_RENDER_TARGET_PROPERTIES {
            r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
            pixelFormat: D2D1_PIXEL_FORMAT {
                format: DXGI_FORMAT_B8G8R8A8_UNORM,
                alphaMode: D2D1_ALPHA_MODE_IGNORE,
            },
            dpiX: 96.0,
            dpiY: 96.0,
            usage: D2D1_RENDER_TARGET_USAGE_NONE,
            minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
        };
        let render_target = renderer
            .d2d_factory
            .CreateDCRenderTarget(&render_properties)?;
        let full_rect = RECT {
            left: 0,
            top: 0,
            right: width,
            bottom: height,
        };
        render_target.BindDC(memory_dc, &full_rect)?;
        render_target.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
        render_target.SetTextRenderingParams(&renderer.rendering_params);
        let white = D2D1_COLOR_F {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        };
        let black = D2D1_COLOR_F {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        };
        let brush = render_target.CreateSolidColorBrush(&white, None)?;
        let layout = D2D_RECT_F {
            left: rect.left as f32,
            top: rect.top as f32,
            right: rect.right as f32,
            bottom: rect.bottom as f32,
        };
        let utf16_text = text.encode_utf16().collect::<Vec<_>>();
        render_target.BeginDraw();
        render_target.Clear(Some(&black));
        render_target.DrawText(
            &utf16_text,
            &text_format,
            &layout,
            &brush,
            D2D1_DRAW_TEXT_OPTIONS_CLIP,
            DWRITE_MEASURING_MODE_NATURAL,
        );
        render_target.EndDraw(None, None)?;

        let mask = std::slice::from_raw_parts(bits.cast::<u8>(), (width * height * 4) as usize);
        for y in rect.top.max(0)..rect.bottom.min(height) {
            for x in rect.left.max(0)..rect.right.min(width) {
                let offset = ((y * width + x) * 4) as usize;
                let coverage = mask[offset].max(mask[offset + 1]).max(mask[offset + 2]);
                composite_pixel(pixels, width, x, y, style.color, coverage);
            }
        }

        let _ = SelectObject(memory_dc, previous_bitmap);
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        let _ = DeleteDC(memory_dc);
        let _ = ReleaseDC(None, screen_dc);
        Ok(())
    }
}

unsafe fn present_layered_pixels(
    window: HWND,
    width: i32,
    height: i32,
    pixels: &[u8],
    alpha: u8,
) -> WinResult<()> {
    // SAFETY: The DIB owns `bits` until UpdateLayeredWindow copies the pixels.
    unsafe {
        let screen_dc = GetDC(None);
        let memory_dc = CreateCompatibleDC(Some(screen_dc));
        let bitmap_info = top_down_bitmap_info(width, height);
        let mut bits: *mut c_void = ptr::null_mut();
        let bitmap = CreateDIBSection(
            Some(screen_dc),
            &bitmap_info,
            DIB_RGB_COLORS,
            &mut bits,
            None,
            0,
        )?;
        ptr::copy_nonoverlapping(pixels.as_ptr(), bits.cast::<u8>(), pixels.len());
        let previous_bitmap = SelectObject(memory_dc, HGDIOBJ(bitmap.0));
        let mut bounds = RECT::default();
        GetWindowRect(window, &mut bounds)?;
        let destination = POINT {
            x: bounds.left,
            y: bounds.top,
        };
        let size = SIZE {
            cx: width,
            cy: height,
        };
        let source = POINT { x: 0, y: 0 };
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: alpha,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        let result = UpdateLayeredWindow(
            window,
            Some(screen_dc),
            Some(&destination),
            Some(&size),
            Some(memory_dc),
            Some(&source),
            COLORREF(0),
            Some(&blend),
            ULW_ALPHA,
        );
        let _ = SelectObject(memory_dc, previous_bitmap);
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        let _ = DeleteDC(memory_dc);
        let _ = ReleaseDC(None, screen_dc);
        result
    }
}

fn top_down_bitmap_info(width: i32, height: i32) -> BITMAPINFO {
    BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    }
}

unsafe fn render_pill(
    window: HWND,
    width: i32,
    height: i32,
    dpi: u32,
    notes: &[NoteSummary],
) -> WinResult<()> {
    let colors = notes
        .iter()
        .map(|note| dot_color(&note.color))
        .collect::<Vec<_>>();
    let pixels = render_pill_pixels(width, height, dpi, &colors);

    // SAFETY: The DIB section owns `bits` until UpdateLayeredWindow copies it.
    unsafe {
        let screen_dc = GetDC(None);
        let memory_dc = CreateCompatibleDC(Some(screen_dc));
        let bitmap_info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut c_void = ptr::null_mut();
        let bitmap = CreateDIBSection(
            Some(screen_dc),
            &bitmap_info,
            DIB_RGB_COLORS,
            &mut bits,
            None,
            0,
        )?;
        ptr::copy_nonoverlapping(pixels.as_ptr(), bits.cast::<u8>(), pixels.len());

        let previous_bitmap = SelectObject(memory_dc, HGDIOBJ(bitmap.0));
        let mut bounds = RECT::default();
        GetWindowRect(window, &mut bounds)?;
        let destination = POINT {
            x: bounds.left,
            y: bounds.top,
        };
        let size = SIZE {
            cx: width,
            cy: height,
        };
        let source = POINT { x: 0, y: 0 };
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: 255,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        let result = UpdateLayeredWindow(
            window,
            Some(screen_dc),
            Some(&destination),
            Some(&size),
            Some(memory_dc),
            Some(&source),
            COLORREF(0),
            Some(&blend),
            ULW_ALPHA,
        );
        let _ = SelectObject(memory_dc, previous_bitmap);
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
        let _ = DeleteDC(memory_dc);
        let _ = ReleaseDC(None, screen_dc);
        result
    }
}

fn render_pill_pixels(width: i32, height: i32, dpi: u32, dots: &[Color]) -> Vec<u8> {
    let scale = dpi as f32 / 96.0;
    let border = Color::new(215, 208, 198);
    let surface = Color::new(242, 238, 232);
    let capsule_right = width as f32 - PILL_CAPSULE_RIGHT_MARGIN_DIP as f32 * scale;
    let capsule_left = capsule_right - PILL_CAPSULE_WIDTH_DIP as f32 * scale;
    let capsule_top = PILL_CAPSULE_VERTICAL_INSET_DIP as f32 * scale;
    let capsule_bottom = height as f32 - PILL_CAPSULE_VERTICAL_INSET_DIP as f32 * scale;
    let displayed_dots = if dots.is_empty() {
        vec![dot_color("yellow")]
    } else {
        dots.iter().take(PILL_MAX_DOTS).copied().collect()
    };
    let dot_layout = dot_layout(displayed_dots.len(), scale, capsule_top, capsule_bottom);
    let sample_count = (ANTIALIAS_SAMPLES * ANTIALIAS_SAMPLES) as u32;
    let mut pixels = vec![0; (width * height * 4) as usize];

    for y in 0..height {
        for x in 0..width {
            let mut red = 0_u32;
            let mut green = 0_u32;
            let mut blue = 0_u32;
            let mut alpha = 0_u32;
            for sample_y in 0..ANTIALIAS_SAMPLES {
                for sample_x in 0..ANTIALIAS_SAMPLES {
                    let px = x as f32 + (sample_x as f32 + 0.5) / ANTIALIAS_SAMPLES as f32;
                    let py = y as f32 + (sample_y as f32 + 0.5) / ANTIALIAS_SAMPLES as f32;
                    let mut color = inside_capsule_bounds(
                        px,
                        py,
                        capsule_left,
                        capsule_top,
                        capsule_right,
                        capsule_bottom,
                        0.2,
                    )
                    .then_some(border);
                    if inside_capsule_bounds(
                        px,
                        py,
                        capsule_left,
                        capsule_top,
                        capsule_right,
                        capsule_bottom,
                        scale,
                    ) {
                        color = Some(surface);
                    }
                    for ((center_y, radius), dot_color) in dot_layout.iter().zip(&displayed_dots) {
                        if inside_circle(
                            px,
                            py,
                            (capsule_left + capsule_right) / 2.0,
                            *center_y,
                            *radius,
                        ) {
                            color = Some(*dot_color);
                        }
                    }
                    if let Some(color) = color {
                        red += color.red as u32;
                        green += color.green as u32;
                        blue += color.blue as u32;
                        alpha += 255;
                    }
                }
            }
            let offset = ((y * width + x) * 4) as usize;
            pixels[offset] = (blue / sample_count) as u8;
            pixels[offset + 1] = (green / sample_count) as u8;
            pixels[offset + 2] = (red / sample_count) as u8;
            pixels[offset + 3] = (alpha / sample_count) as u8;
        }
    }
    pixels
}

fn dot_layout(count: usize, scale: f32, top: f32, bottom: f32) -> Vec<(f32, f32)> {
    let count = count.min(PILL_MAX_DOTS);
    if count == 0 {
        return Vec::new();
    }

    let diameter = PILL_DOT_SIZE_DIP as f32 * scale;
    let spacing = PILL_DOT_SPACING_DIP as f32 * scale;
    let content_height = count as f32 * diameter + count.saturating_sub(1) as f32 * spacing;
    let first_center = top + ((bottom - top - content_height) / 2.0) + diameter / 2.0;
    (0..count)
        .map(|index| {
            (
                first_center + index as f32 * (diameter + spacing),
                diameter / 2.0,
            )
        })
        .collect()
}

unsafe fn resize_pill_for_dpi(window: HWND, wparam: WPARAM, lparam: LPARAM) {
    // SAFETY: WM_DPICHANGED supplies a valid suggested RECT pointer.
    unsafe {
        let dpi = (wparam.0 & 0xffff) as u32;
        let suggested = *(lparam.0 as *const RECT);
        let width = scale_for_dpi(PILL_WINDOW_WIDTH_DIP, dpi);
        let height = pill_height(current_notes().len(), dpi);
        let center = POINT {
            x: (suggested.left + suggested.right) / 2,
            y: (suggested.top + suggested.bottom) / 2,
        };
        let Ok(work_area) = work_area_at(center) else {
            return;
        };
        let x = work_area.right - width;
        let y = (center.y - height / 2).clamp(work_area.top, work_area.bottom - height);
        let _ = SetWindowPos(
            window,
            None,
            x,
            y,
            width,
            height,
            SWP_NOACTIVATE | SWP_NOZORDER,
        );
        let _ = render_pill(window, width, height, dpi, &current_notes());
    }
}

unsafe fn move_along_edge(window: HWND) {
    // SAFETY: `window` is the active pill window on this UI thread.
    unsafe {
        let mut cursor = POINT::default();
        let mut bounds = RECT::default();
        if GetCursorPos(&mut cursor).is_err() || GetWindowRect(window, &mut bounds).is_err() {
            return;
        }
        let Ok(work_area) = work_area_at(cursor) else {
            return;
        };
        let width = bounds.right - bounds.left;
        let height = bounds.bottom - bounds.top;
        let y = (cursor.y - DRAG_OFFSET_Y.load(Ordering::Relaxed))
            .clamp(work_area.top, work_area.bottom - height);
        let x = work_area.right - width;
        let _ = SetWindowPos(
            window,
            None,
            x,
            y,
            0,
            0,
            SWP_NOACTIVATE | SWP_NOSIZE | SWP_NOZORDER,
        );
    }
}

unsafe fn work_area_at(point: POINT) -> WinResult<RECT> {
    // SAFETY: `info` has the required size and remains valid for the call.
    unsafe {
        let monitor = MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !GetMonitorInfoW(monitor, &mut info).as_bool() {
            return Err(WinError::from_thread());
        }
        Ok(info.rcWork)
    }
}

fn current_notes() -> Vec<NoteSummary> {
    NOTES
        .get()
        .and_then(|notes| notes.lock().ok().map(|notes| notes.clone()))
        .unwrap_or_default()
}

fn max_scroll_offset(note_count: usize) -> usize {
    note_count.saturating_sub(LIST_VISIBLE_NOTE_LIMIT)
}

fn scroll_offset_after_delta(note_count: usize, current: usize, delta: i32) -> usize {
    let max_offset = max_scroll_offset(note_count);
    let current = current.min(max_offset);
    let steps = (delta.unsigned_abs() as usize).div_ceil(120).max(1);
    if delta > 0 {
        current.saturating_sub(steps)
    } else if delta < 0 {
        current.saturating_add(steps).min(max_offset)
    } else {
        current
    }
}

fn note_page(notes: &[NoteSummary]) -> Vec<NoteSummary> {
    let offset = LIST_SCROLL_OFFSET
        .load(Ordering::Relaxed)
        .min(max_scroll_offset(notes.len()));
    LIST_SCROLL_OFFSET.store(offset, Ordering::Relaxed);
    let end = (offset + LIST_VISIBLE_NOTE_LIMIT).min(notes.len());
    notes[offset..end].to_vec()
}

fn card_hover_progress(note_count: usize) -> Vec<f32> {
    let Some(progress) = CARD_HOVER_PROGRESS.get() else {
        return vec![0.0; note_count];
    };
    let Ok(mut progress) = progress.lock() else {
        return vec![0.0; note_count];
    };
    progress.resize(note_count, 0.0);
    progress.clone()
}

fn advance_card_hover_progress(note_count: usize, hovered: Option<usize>) -> bool {
    let Some(progress) = CARD_HOVER_PROGRESS.get() else {
        return false;
    };
    let Ok(mut progress) = progress.lock() else {
        return false;
    };
    let step = animation_step();
    progress.resize(note_count, 0.0);
    let mut changed = false;
    for (index, value) in progress.iter_mut().enumerate() {
        let target = if Some(index) == hovered { 1.0 } else { 0.0 };
        let next = if target > *value {
            (*value + step).min(target)
        } else {
            (*value - step).max(target)
        };
        if (next - *value).abs() > f32::EPSILON {
            *value = next;
            changed = true;
        }
    }
    changed
}

fn reset_card_hover_progress() {
    if let Some(progress) = CARD_HOVER_PROGRESS.get()
        && let Ok(mut progress) = progress.lock()
    {
        progress.fill(0.0);
    }
}

fn animation_step() -> f32 {
    let Some(last_tick) = LAST_ANIMATION_TICK.get() else {
        return 0.0;
    };
    let Ok(mut last_tick) = last_tick.lock() else {
        return 0.0;
    };
    let now = Instant::now();
    let elapsed = now.duration_since(*last_tick).as_secs_f32();
    *last_tick = now;
    (elapsed / CARD_ANIMATION_DURATION_SECONDS).clamp(0.0, 1.0)
}

fn reset_animation_clock() {
    if let Some(last_tick) = LAST_ANIMATION_TICK.get()
        && let Ok(mut last_tick) = last_tick.lock()
    {
        *last_tick = Instant::now();
    }
}

fn begin_list_reveal(final_x: i32, final_y: i32, width: i32, height: i32, dpi: u32) {
    let Some(animation) = LIST_REVEAL_ANIMATION.get() else {
        return;
    };
    let Ok(mut animation) = animation.lock() else {
        return;
    };
    animation.progress = 0.0;
    animation.target = 1.0;
    animation.last_tick = Instant::now();
    animation.final_x = final_x;
    animation.final_y = final_y;
    animation.width = width;
    animation.height = height;
    animation.dpi = dpi;
}

fn set_list_reveal_target(target: f32) {
    let Some(animation) = LIST_REVEAL_ANIMATION.get() else {
        return;
    };
    let Ok(mut animation) = animation.lock() else {
        return;
    };
    if (animation.target - target).abs() > f32::EPSILON {
        animation.target = target;
        animation.last_tick = Instant::now();
    }
}

fn update_list_reveal_geometry(final_x: i32, final_y: i32, width: i32, height: i32, dpi: u32) {
    let Some(animation) = LIST_REVEAL_ANIMATION.get() else {
        return;
    };
    let Ok(mut animation) = animation.lock() else {
        return;
    };
    animation.final_x = final_x;
    animation.final_y = final_y;
    animation.width = width;
    animation.height = height;
    animation.dpi = dpi;
}

fn advance_list_reveal() -> Option<ListRevealFrame> {
    let animation = LIST_REVEAL_ANIMATION.get()?;
    let mut animation = animation.lock().ok()?;
    let now = Instant::now();
    let step = now.duration_since(animation.last_tick).as_secs_f32() / LIST_REVEAL_DURATION_SECONDS;
    animation.last_tick = now;
    let previous = animation.progress;
    if animation.target > animation.progress {
        animation.progress = (animation.progress + step).min(animation.target);
    } else if animation.target < animation.progress {
        animation.progress = (animation.progress - step).max(animation.target);
    }
    Some(ListRevealFrame {
        progress: animation.progress,
        target: animation.target,
        final_x: animation.final_x,
        final_y: animation.final_y,
        width: animation.width,
        height: animation.height,
        dpi: animation.dpi,
        changed: (animation.progress - previous).abs() > f32::EPSILON,
    })
}

fn list_reveal_alpha() -> u8 {
    let progress = LIST_REVEAL_ANIMATION
        .get()
        .and_then(|animation| animation.lock().ok().map(|animation| animation.progress))
        .unwrap_or(1.0);
    (ease_out_cubic(progress) * 255.0).round() as u8
}

fn ease_out_cubic(progress: f32) -> f32 {
    let progress = progress.clamp(0.0, 1.0);
    1.0 - (1.0 - progress).powi(3)
}

fn begin_list_scroll_animation(delta: i32, visible_note_count: usize) {
    let Some(animation) = LIST_SCROLL_ANIMATION.get() else {
        return;
    };
    let Ok(mut animation) = animation.lock() else {
        return;
    };
    animation.progress = 0.0;
    animation.direction = if delta < 0 { 1 } else { -1 };
    animation.highlighted_card = if delta < 0 {
        visible_note_count.checked_sub(1)
    } else if visible_note_count == 0 {
        None
    } else {
        Some(0)
    };
    animation.last_tick = Instant::now();
    animation.active = true;
}

fn advance_list_scroll_animation() -> bool {
    let Some(animation) = LIST_SCROLL_ANIMATION.get() else {
        return false;
    };
    let Ok(mut animation) = animation.lock() else {
        return false;
    };
    if !animation.active {
        return false;
    }

    let now = Instant::now();
    let step = now.duration_since(animation.last_tick).as_secs_f32()
        / LIST_SCROLL_ANIMATION_DURATION_SECONDS;
    animation.last_tick = now;
    animation.progress = (animation.progress + step).min(1.0);
    if animation.progress >= 1.0 {
        animation.active = false;
        animation.highlighted_card = None;
    }
    true
}

fn list_scroll_visual(dpi: u32) -> ListScrollVisual {
    let Some(animation) = LIST_SCROLL_ANIMATION.get() else {
        return ListScrollVisual {
            offset_y: 0.0,
            highlighted_card: None,
            card_shift: 0.0,
        };
    };
    let Ok(animation) = animation.lock() else {
        return ListScrollVisual {
            offset_y: 0.0,
            highlighted_card: None,
            card_shift: 0.0,
        };
    };
    if !animation.active {
        return ListScrollVisual {
            offset_y: 0.0,
            highlighted_card: None,
            card_shift: 0.0,
        };
    }

    let remaining = 1.0 - ease_out_cubic(animation.progress);
    ListScrollVisual {
        offset_y: animation.direction as f32
            * scale_for_dpi(LIST_SCROLL_SLIDE_DIP, dpi) as f32
            * remaining,
        highlighted_card: animation.highlighted_card,
        card_shift: scale_for_dpi(CARD_HOVER_SHIFT_DIP, dpi) as f32 * remaining,
    }
}

fn card_left(index: usize, dpi: u32, progress: &[f32]) -> i32 {
    scale_for_dpi(17, dpi) - card_shift(index, dpi, progress)
}

fn card_shift(index: usize, dpi: u32, progress: &[f32]) -> i32 {
    card_shift_f32(index, dpi, progress).round() as i32
}

fn card_shift_f32(index: usize, dpi: u32, progress: &[f32]) -> f32 {
    let linear = progress.get(index).copied().unwrap_or(0.0).clamp(0.0, 1.0);
    let eased = ease_out_cubic(linear);
    scale_for_dpi(CARD_HOVER_SHIFT_DIP, dpi) as f32 * eased
}

fn list_height(note_count: usize, dpi: u32) -> i32 {
    footer_y(note_count, dpi) + scale_for_dpi(58, dpi)
}

fn pill_height(note_count: usize, dpi: u32) -> i32 {
    let logical_height =
        (note_count as i32 * 9 + 24).clamp(PILL_MIN_HEIGHT_DIP, PILL_MAX_HEIGHT_DIP);
    scale_for_dpi(logical_height, dpi)
}

fn footer_y(note_count: usize, dpi: u32) -> i32 {
    let y = if note_count == 0 {
        51
    } else {
        39 + (note_count.saturating_sub(1) as i32 * 48) + 65 + 12
    };
    scale_for_dpi(y, dpi)
}

fn card_colors(color: &str) -> (Color, Color) {
    match color {
        "green" => (Color::new(197, 221, 185), Color::new(166, 199, 150)),
        "yellow" => (Color::new(247, 230, 166), Color::new(225, 204, 127)),
        _ => (Color::new(222, 212, 239), Color::new(194, 175, 221)),
    }
}

fn dot_color(color: &str) -> Color {
    match color {
        "green" => Color::new(140, 167, 123),
        "purple" => Color::new(167, 145, 205),
        _ => Color::new(213, 175, 70),
    }
}

fn close_app() {
    // SAFETY: Both handles, when non-null, belong to this UI thread.
    unsafe {
        let pill = load_window(&PILL_WINDOW);
        let list = load_window(&LIST_WINDOW);
        if !pill.0.is_null() {
            let _ = DestroyWindow(pill);
        }
        if !list.0.is_null() {
            let _ = DestroyWindow(list);
        }
    }
}

fn set_hand_cursor() -> LRESULT {
    // SAFETY: IDC_HAND is a shared system cursor and does not need to be destroyed.
    unsafe {
        if let Ok(cursor) = LoadCursorW(None, IDC_HAND) {
            let _ = SetCursor(Some(cursor));
        }
    }
    LRESULT(1)
}

fn load_window(storage: &AtomicIsize) -> HWND {
    HWND(storage.load(Ordering::Acquire) as *mut c_void)
}

fn utf16(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(Some(0)).collect()
}

fn inside_capsule_bounds(
    x: f32,
    y: f32,
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
    inset: f32,
) -> bool {
    let left = left + inset;
    let right = right - inset;
    let top = top + inset;
    let bottom = bottom - inset;
    let radius = (right - left) / 2.0;
    let center_x = (left + right) / 2.0;
    let center_y = y.clamp(top + radius, bottom - radius);
    let dx = x - center_x;
    let dy = y - center_y;
    dx * dx + dy * dy <= radius * radius
}

fn inside_circle(x: f32, y: f32, center_x: f32, center_y: f32, radius: f32) -> bool {
    let dx = x - center_x;
    let dy = y - center_y;
    dx * dx + dy * dy <= radius * radius
}

impl Color {
    const fn new(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue }
    }
}

const fn scale_for_dpi(value: i32, dpi: u32) -> i32 {
    (value * dpi as i32 + 48) / 96
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hover_shift_keeps_fractional_easing_positions() {
        let progress = [0.1_f32];
        let shift = card_shift_f32(0, 96, &progress);

        assert!(shift > 2.4 && shift < 2.5);
        assert_eq!(card_shift_f32(0, 96, &[0.0]), 0.0);
        assert_eq!(card_shift_f32(0, 96, &[1.0]), 9.0);
    }

    #[test]
    fn pill_height_tracks_note_count_and_caps_at_nine_notes() {
        assert_eq!(pill_height(2, 96), 48);
        assert_eq!(pill_height(3, 96), 51);
        assert_eq!(pill_height(6, 96), 78);
        assert_eq!(pill_height(9, 96), 94);
        assert_eq!(pill_height(20, 96), 94);
    }

    #[test]
    fn pill_dots_keep_the_reference_size_spacing_and_limit() {
        let layout = dot_layout(9, 1.0, 4.0, 90.0);
        assert_eq!(layout.len(), 7);
        assert_eq!(layout[0].1, 2.5);
        assert_eq!(layout[1].0 - layout[0].0, 9.0);
        assert!((layout[0].0 + layout[6].0 - 94.0).abs() < f32::EPSILON);
    }

    #[test]
    fn list_scroll_range_starts_after_nine_notes() {
        assert_eq!(max_scroll_offset(9), 0);
        assert_eq!(max_scroll_offset(10), 1);
        assert_eq!(max_scroll_offset(15), 6);
        assert_eq!(scroll_offset_after_delta(12, 0, -120), 1);
        assert_eq!(scroll_offset_after_delta(12, 1, 120), 0);
        assert_eq!(scroll_offset_after_delta(12, 2, -240), 3);
        assert_eq!(scroll_offset_after_delta(12, 3, -120), 3);
    }
}
