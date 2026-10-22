use std::{
    cell::RefCell,
    collections::HashMap,
    error::Error,
    ffi::c_void,
    ptr,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicI32, AtomicIsize, Ordering},
    },
    time::{Duration, Instant},
};

use windows::{
    Win32::{
        Foundation::{
            COLORREF, E_INVALIDARG, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM,
        },
        Graphics::{
            Direct2D::{
                Common::{D2D_RECT_F, D2D1_ALPHA_MODE_IGNORE, D2D1_COLOR_F, D2D1_PIXEL_FORMAT},
                D2D1_DRAW_TEXT_OPTIONS_CLIP, D2D1_ELLIPSE, D2D1_FACTORY_TYPE_SINGLE_THREADED,
                D2D1_FEATURE_LEVEL_DEFAULT, D2D1_RENDER_TARGET_PROPERTIES,
                D2D1_RENDER_TARGET_TYPE_DEFAULT, D2D1_RENDER_TARGET_USAGE_NONE,
                D2D1_TEXT_ANTIALIAS_MODE_CLEARTYPE, D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE,
                D2D1CreateFactory, ID2D1DCRenderTarget, ID2D1Factory,
            },
            DirectWrite::{
                DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_WEIGHT, DWRITE_MEASURING_MODE_NATURAL, DWRITE_PARAGRAPH_ALIGNMENT,
                DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_PIXEL_GEOMETRY_FLAT,
                DWRITE_PIXEL_GEOMETRY_RGB, DWRITE_RENDERING_MODE_CLEARTYPE_NATURAL_SYMMETRIC,
                DWRITE_RENDERING_MODE_NATURAL_SYMMETRIC, DWRITE_TEXT_ALIGNMENT,
                DWRITE_TEXT_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_LEADING,
                DWRITE_TEXT_ALIGNMENT_TRAILING, DWRITE_TRIMMING,
                DWRITE_TRIMMING_GRANULARITY_CHARACTER, DWRITE_WORD_WRAPPING,
                DWRITE_WORD_WRAPPING_NO_WRAP, DWriteCreateFactory, IDWriteFactory,
                IDWriteRenderingParams,
            },
            Dwm::{
                DWMWA_BORDER_COLOR, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
                DwmExtendFrameIntoClientArea, DwmSetWindowAttribute,
            },
            Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM,
            Gdi::{
                AC_SRC_ALPHA, AC_SRC_OVER, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION,
                BeginPaint, BitBlt, CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, CreateCompatibleDC,
                CreateDIBSection, CreateFontW, CreateRoundRectRgn, CreateSolidBrush,
                DEFAULT_CHARSET, DEFAULT_PITCH, DIB_RGB_COLORS, DeleteDC, DeleteObject, EndPaint,
                FF_DONTCARE, GetDC, GetMonitorInfoW, HBITMAP, HDC, HFONT, HGDIOBJ, InvalidateRect,
                MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromPoint, OPAQUE,
                OUT_DEFAULT_PRECIS, PAINTSTRUCT, RDW_ALLCHILDREN, RDW_INVALIDATE, RDW_UPDATENOW,
                RedrawWindow, ReleaseDC, SRCCOPY, SelectObject, SetBkColor, SetBkMode,
                SetTextColor, SetWindowRgn,
            },
        },
        System::LibraryLoader::GetModuleHandleW,
        UI::{
            Controls::MARGINS,
            HiDpi::{
                DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForSystem, GetDpiForWindow,
                SetProcessDpiAwarenessContext,
            },
            Input::KeyboardAndMouse::{GetFocus, ReleaseCapture, SetCapture, SetFocus},
            WindowsAndMessaging::{
                AppendMenuW, CS_HREDRAW, CS_VREDRAW, CreatePopupMenu, CreateWindowExW,
                DefWindowProcW, DestroyMenu, DestroyWindow, DispatchMessageW, EN_CHANGE,
                EN_KILLFOCUS, EN_SETFOCUS, ES_AUTOHSCROLL, ES_AUTOVSCROLL, ES_MULTILINE,
                ES_NOHIDESEL, ES_WANTRETURN, GWL_EXSTYLE, GetClientRect, GetCursorPos, GetMessageW,
                GetWindowLongPtrW, GetWindowRect, GetWindowTextLengthW, GetWindowTextW, HMENU,
                HTCAPTION, HTCLIENT, IDC_ARROW, IDC_HAND, IDYES, IsWindowVisible, KillTimer,
                LWA_ALPHA, LoadCursorW, MB_DEFBUTTON2, MB_ICONWARNING, MB_YESNO, MF_SEPARATOR,
                MF_STRING, MSG, MessageBoxW, PostQuitMessage, RegisterClassW,
                SPI_GETCLIENTAREAANIMATION, SW_HIDE, SW_SHOW, SW_SHOWNOACTIVATE, SWP_NOACTIVATE,
                SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
                SendMessageW, SetCursor, SetForegroundWindow, SetLayeredWindowAttributes, SetTimer,
                SetWindowLongPtrW, SetWindowPos, SetWindowTextW, ShowWindow, SystemParametersInfoW,
                TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu, TranslateMessage, ULW_ALPHA,
                UpdateLayeredWindow, WA_INACTIVE, WINDOW_EX_STYLE, WINDOW_STYLE, WM_ACTIVATE,
                WM_APP, WM_CLOSE, WM_COMMAND, WM_CTLCOLOREDIT, WM_CTLCOLORSTATIC, WM_DESTROY,
                WM_DPICHANGED, WM_ERASEBKGND, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE,
                WM_MOUSEWHEEL, WM_NCCALCSIZE, WM_NCHITTEST, WM_NCLBUTTONDOWN, WM_PAINT,
                WM_RBUTTONUP, WM_SETCURSOR, WM_SETFONT, WM_TIMER, WNDCLASSW, WS_CAPTION, WS_CHILD,
                WS_CLIPCHILDREN, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
                WS_POPUP, WS_TABSTOP, WS_THICKFRAME, WS_VISIBLE,
            },
        },
    },
    core::{BOOL, Error as WinError, PCWSTR, Result as WinResult},
};
use windows_numerics::Vector2;

use crate::db::{Database, NoteSummary};

mod animation_clock;
use animation_clock::AnimationClock;

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
const LIST_VISIBLE_NOTE_LIMIT: usize = 6;
const ANTIALIAS_SAMPLES: i32 = 8;
const CARD_HOVER_SHIFT_DIP: i32 = 9;
const CARD_ANIMATION_DURATION_SECONDS: f32 = 0.15;
const LIST_REVEAL_OFFSET_DIP: i32 = 18;
const LIST_REVEAL_DURATION_SECONDS: f32 = 0.18;
const WM_ANIMATION_FRAME: u32 = WM_APP + 1;
const LIST_SCROLL_SMOOTH_TIME_SECONDS: f32 = 0.12;
const LIST_SCROLL_STEP_DIP: i32 = 48;
const UI_FONT_FACE: &str = "Microsoft YaHei UI";
const SYMBOL_FONT_FACE: &str = "Segoe UI";
const DECK_LABEL_FONT_DIP: i32 = 10;
const CARD_LABEL_FONT_DIP: i32 = 12;
const NORMAL_FONT_WEIGHT: i32 = 400;
const MEDIUM_FONT_WEIGHT: i32 = 500;
const EDITOR_WIDTH_DIP: i32 = 360;
const EDITOR_HEIGHT_DIP: i32 = 480;
const EDITOR_PANEL_LEFT_DIP: i32 = 30;
const EDITOR_GAP_DIP: i32 = 6;
const EDITOR_CHROME_FONT_DIP: i32 = 10;
const EDITOR_SIDEBAR_COLOR: Color = Color::new(198, 218, 236);
const EDITOR_BORDER_COLOR: Color = Color::new(185, 207, 228);
const EDITOR_REVEAL_DURATION: Duration = Duration::from_millis(200);
const EDITOR_REVEAL_OFFSET_DIP: i32 = 12;
const EDITOR_TITLE_ID: usize = 1001;
const EDITOR_BODY_ID: usize = 1002;
const EDITOR_PLACEHOLDER_ID: usize = 1003;
const EDITOR_TITLE_PLACEHOLDER_ID: usize = 1004;
const EDITOR_SAVE_TIMER_ID: usize = 2;
const NOTE_MENU_TOGGLE_PIN_ID: usize = 2001;
const NOTE_MENU_DELETE_ID: usize = 2002;

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
static DRAGGING: AtomicBool = AtomicBool::new(false);
static DRAG_OFFSET_Y: AtomicI32 = AtomicI32::new(0);
static EDITOR_WINDOW: AtomicIsize = AtomicIsize::new(0);
static EDITOR_TITLE_EDIT: AtomicIsize = AtomicIsize::new(0);
static EDITOR_BODY_EDIT: AtomicIsize = AtomicIsize::new(0);
static EDITOR_PLACEHOLDER: AtomicIsize = AtomicIsize::new(0);
static EDITOR_TITLE_PLACEHOLDER: AtomicIsize = AtomicIsize::new(0);
static EDITOR_BACKGROUND_BRUSH: AtomicIsize = AtomicIsize::new(0);
static EDITOR_TITLE_FONT: AtomicIsize = AtomicIsize::new(0);
static EDITOR_BODY_FONT: AtomicIsize = AtomicIsize::new(0);
static EDITOR_NATIVE_CORNERS: AtomicBool = AtomicBool::new(false);
static EDITOR_LOADING: AtomicBool = AtomicBool::new(false);
static EDITOR_DIRTY: AtomicBool = AtomicBool::new(false);
static EDITOR_CLOSING: AtomicBool = AtomicBool::new(false);
static EDITOR_MINIMIZED_TO_LIST: AtomicBool = AtomicBool::new(false);
static LIST_CONTEXT_MENU_ACTIVE: AtomicBool = AtomicBool::new(false);
static ACTIVE_NOTE_ID: OnceLock<Mutex<Option<String>>> = OnceLock::new();
static ACTIVE_NOTE_COLOR: OnceLock<Mutex<String>> = OnceLock::new();

thread_local! {
    static EDITOR_RENDERER: RefCell<Option<EditorRenderer>> = const { RefCell::new(None) };
    static CARD_SHAPE_CACHE: RefCell<HashMap<CardShapeKey, Arc<Vec<CardSprite>>>> = RefCell::new(HashMap::new());
    static ANIMATION_CLOCK: RefCell<Option<AnimationClock>> = const { RefCell::new(None) };
    static EDITOR_REVEAL: RefCell<Option<EditorReveal>> = const { RefCell::new(None) };
}

struct EditorReveal {
    started: Instant,
    target: POINT,
    offset: i32,
    original_style: isize,
}

impl EditorReveal {
    fn frame(&self, elapsed: Duration) -> (POINT, u8, bool) {
        let progress =
            (elapsed.as_secs_f32() / EDITOR_REVEAL_DURATION.as_secs_f32()).clamp(0.0, 1.0);
        let eased = ease_out_cubic(progress);
        (
            POINT {
                x: self.target.x + (self.offset as f32 * (1.0 - eased)).round() as i32,
                y: self.target.y,
            },
            (255.0 * eased).round() as u8,
            progress >= 1.0,
        )
    }
}

#[derive(Clone, Copy)]
struct Color {
    red: u8,
    green: u8,
    blue: u8,
}

#[derive(Clone, Copy)]
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
    cleartype_rendering_params: IDWriteRenderingParams,
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
            let cleartype_rendering_params = write_factory.CreateCustomRenderingParams(
                2.2,
                1.0,
                1.0,
                DWRITE_PIXEL_GEOMETRY_RGB,
                DWRITE_RENDERING_MODE_CLEARTYPE_NATURAL_SYMMETRIC,
            )?;
            Ok(Self {
                d2d_factory,
                write_factory,
                rendering_params,
                cleartype_rendering_params,
            })
        }
    }
}

struct OpaqueSurface {
    dc: HDC,
    bitmap: HBITMAP,
    previous: HGDIOBJ,
    width: i32,
    height: i32,
}

impl OpaqueSurface {
    unsafe fn new(width: i32, height: i32) -> WinResult<Self> {
        // SAFETY: The memory DC owns its bitmap until Drop restores the previous object.
        unsafe {
            let dc = CreateCompatibleDC(None);
            if dc.0.is_null() {
                return Err(WinError::from_thread());
            }
            let mut bits = ptr::null_mut();
            let bitmap = match CreateDIBSection(
                Some(dc),
                &top_down_bitmap_info(width, height),
                DIB_RGB_COLORS,
                &mut bits,
                None,
                0,
            ) {
                Ok(bitmap) => bitmap,
                Err(error) => {
                    let _ = DeleteDC(dc);
                    return Err(error);
                }
            };
            let previous = SelectObject(dc, HGDIOBJ(bitmap.0));
            if previous.0.is_null() || previous.0 as isize == -1 {
                let error = WinError::from_thread();
                let _ = DeleteObject(HGDIOBJ(bitmap.0));
                let _ = DeleteDC(dc);
                return Err(error);
            }
            Ok(Self {
                dc,
                bitmap,
                previous,
                width,
                height,
            })
        }
    }
}

impl Drop for OpaqueSurface {
    fn drop(&mut self) {
        // SAFETY: The bitmap is deselected before releasing the owned GDI objects.
        unsafe {
            let _ = SelectObject(self.dc, self.previous);
            let _ = DeleteObject(HGDIOBJ(self.bitmap.0));
            let _ = DeleteDC(self.dc);
        }
    }
}

struct EditorRenderer {
    rasterizer: TextRasterizer,
    target: ID2D1DCRenderTarget,
    surface: OpaqueSurface,
}

impl EditorRenderer {
    unsafe fn new(width: i32, height: i32) -> WinResult<Self> {
        // SAFETY: These resources remain on the owning window's UI thread.
        unsafe {
            let rasterizer = TextRasterizer::new()?;
            let target =
                rasterizer
                    .d2d_factory
                    .CreateDCRenderTarget(&D2D1_RENDER_TARGET_PROPERTIES {
                        r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
                        pixelFormat: D2D1_PIXEL_FORMAT {
                            format: DXGI_FORMAT_B8G8R8A8_UNORM,
                            alphaMode: D2D1_ALPHA_MODE_IGNORE,
                        },
                        dpiX: 96.0,
                        dpiY: 96.0,
                        usage: D2D1_RENDER_TARGET_USAGE_NONE,
                        minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
                    })?;
            let surface = OpaqueSurface::new(width, height)?;
            Ok(Self {
                rasterizer,
                target,
                surface,
            })
        }
    }

    unsafe fn render(&self, client: RECT, dpi: u32) -> WinResult<()> {
        // SAFETY: All rendering uses the same offscreen DC in one D2D drawing pass.
        unsafe {
            self.target.BindDC(self.surface.dc, &client)?;
            self.target.BeginDraw();
            let result = paint_editor_background(&self.target, client, dpi)
                .and_then(|()| paint_editor_symbols(&self.target, client, dpi))
                .and_then(|()| paint_editor_text(self, client, dpi));
            let end_result = self.target.EndDraw(None, None);
            result.and(end_result)
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
    shape: CardShapeKey,
}

#[derive(Clone, Hash, PartialEq, Eq)]
struct CardShapeKey {
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
    position: f32,
    target: f32,
    velocity: f32,
    step: f32,
    last_tick: Instant,
    active: bool,
}

struct NoteViewport {
    notes: Vec<NoteSummary>,
    offset_y: f32,
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
    fn layout_matches(&self, width: i32, height: i32, dpi: u32, total_note_count: usize) -> bool {
        self.width == width
            && self.height == height
            && self.dpi == dpi
            && self.total_note_count == total_note_count
    }

    fn matches(
        &self,
        width: i32,
        height: i32,
        dpi: u32,
        notes: &[NoteSummary],
        total_note_count: usize,
    ) -> bool {
        self.layout_matches(width, height, dpi, total_note_count)
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
            position: 0.0,
            target: 0.0,
            velocity: 0.0,
            step: LIST_SCROLL_STEP_DIP as f32,
            last_tick: Instant::now(),
            active: false,
        }))
        .map_err(|_| std::io::Error::other("列表滚动动画状态已初始化"))?;
    ACTIVE_NOTE_ID
        .set(Mutex::new(None))
        .map_err(|_| std::io::Error::other("编辑器状态已初始化"))?;
    ACTIVE_NOTE_COLOR
        .set(Mutex::new(String::from("purple")))
        .map_err(|_| std::io::Error::other("编辑器颜色状态已初始化"))?;

    // SAFETY: DPI awareness is configured before creating windows. All HWNDs
    // and GDI resources are used on this UI thread.
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)?;
        let instance = HINSTANCE(GetModuleHandleW(None)?.0);

        let pill_class = utf16("NotyFloatingPill");
        let list_class = utf16("NotyNoteList");
        let editor_class = utf16("NotyEditorWindow");
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
        register_editor_window_class(instance, PCWSTR(editor_class.as_ptr()), editor_window_proc)?;

        let dpi = GetDpiForSystem();
        let editor_brush = CreateSolidBrush(rgb(252, 249, 241));
        EDITOR_BACKGROUND_BRUSH.store(editor_brush.0 as isize, Ordering::Release);
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
        let viewport = note_viewport(&notes, dpi);
        let list_height = list_height(notes.len(), dpi);
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
        let clock = AnimationClock::new(list_window, WM_ANIMATION_FRAME)?;
        ANIMATION_CLOCK.with(|slot| *slot.borrow_mut() = Some(clock));

        render_note_list(
            list_window,
            scale_for_dpi(LIST_WIDTH_DIP, dpi),
            list_height,
            dpi,
            &viewport.notes,
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

unsafe fn register_editor_window_class(
    instance: HINSTANCE,
    class_name: PCWSTR,
    window_proc: unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT,
) -> WinResult<()> {
    // SAFETY: The class name remains alive for the entire message loop.
    unsafe {
        let window_class = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            hCursor: LoadCursorW(None, IDC_ARROW)?,
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
            WM_ANIMATION_FRAME => {
                advance_editor_reveal();
                update_note_list_interaction(window);
                ANIMATION_CLOCK.with(|slot| {
                    if let Some(clock) = slot.borrow().as_ref() {
                        clock.frame_handled();
                    }
                });
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
                handle_list_context_menu(window, lparam);
                LRESULT(0)
            }
            WM_DESTROY => {
                ANIMATION_CLOCK.with(|slot| *slot.borrow_mut() = None);
                LRESULT(0)
            }
            _ => DefWindowProcW(window, message, wparam, lparam),
        }
    }
}

unsafe extern "system" fn editor_window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // SAFETY: Windows calls this procedure on the thread that owns `window`.
    unsafe {
        match message {
            WM_NCCALCSIZE => LRESULT(0),
            WM_NCHITTEST => LRESULT(HTCLIENT as isize),
            WM_ACTIVATE if wparam.0 & 0xffff == WA_INACTIVE as usize => {
                if !LIST_CONTEXT_MENU_ACTIVE.load(Ordering::Acquire)
                    && !cursor_inside_editor_group(window)
                {
                    close_note_editor(window);
                }
                LRESULT(0)
            }
            WM_ERASEBKGND => LRESULT(1),
            WM_PAINT => {
                paint_editor(window);
                LRESULT(0)
            }
            WM_DPICHANGED => {
                finish_editor_reveal(window, false);
                let bounds = &*(lparam.0 as *const RECT);
                let result = SetWindowPos(
                    window,
                    None,
                    bounds.left,
                    bounds.top,
                    bounds.right - bounds.left,
                    bounds.bottom - bounds.top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                )
                .and_then(|()| update_editor_layout(window, wparam.0 as u16 as u32));
                if let Err(error) = result {
                    eprintln!("Failed to update editor DPI: {error}");
                }
                LRESULT(0)
            }
            WM_CTLCOLOREDIT | WM_CTLCOLORSTATIC => {
                let dc = HDC(wparam.0 as *mut c_void);
                let control = HWND(lparam.0 as *mut c_void);
                let placeholder = load_window(&EDITOR_PLACEHOLDER);
                let title_placeholder = load_window(&EDITOR_TITLE_PLACEHOLDER);
                let title_edit = load_window(&EDITOR_TITLE_EDIT);
                let _ = SetBkColor(dc, rgb(252, 249, 241));
                let text_color = if control == placeholder || control == title_placeholder {
                    rgb(178, 168, 151)
                } else if control == title_edit {
                    rgb(61, 57, 49)
                } else {
                    rgb(81, 75, 64)
                };
                let _ = SetTextColor(dc, text_color);
                let _ = SetBkMode(dc, OPAQUE);
                LRESULT(EDITOR_BACKGROUND_BRUSH.load(Ordering::Acquire))
            }
            WM_COMMAND => {
                handle_editor_command(window, wparam);
                LRESULT(0)
            }
            WM_TIMER if wparam.0 == EDITOR_SAVE_TIMER_ID => {
                let _ = KillTimer(Some(window), EDITOR_SAVE_TIMER_ID);
                save_active_note();
                LRESULT(0)
            }
            WM_LBUTTONDOWN => {
                finish_editor_reveal(window, false);
                let x = lparam.0 as i16 as i32;
                let y = (lparam.0 >> 16) as i16 as i32;
                let dpi = GetDpiForWindow(window);
                if y >= scale_for_dpi(14, dpi)
                    && y < scale_for_dpi(46, dpi)
                    && x < scale_for_dpi(238, dpi)
                {
                    let _ = ReleaseCapture();
                    let _ = SendMessageW(
                        window,
                        WM_NCLBUTTONDOWN,
                        Some(WPARAM(HTCAPTION as usize)),
                        Some(LPARAM(0)),
                    );
                }
                LRESULT(0)
            }
            WM_LBUTTONUP => {
                let x = lparam.0 as i16 as i32;
                let y = (lparam.0 >> 16) as i16 as i32;
                let dpi = GetDpiForWindow(window);
                if x >= scale_for_dpi(EDITOR_PANEL_LEFT_DIP + 16, dpi)
                    && x < scale_for_dpi(EDITOR_WIDTH_DIP - 16, dpi)
                    && y >= scale_for_dpi(60, dpi)
                    && y < scale_for_dpi(97, dpi)
                {
                    focus_editor_control(load_window(&EDITOR_TITLE_EDIT));
                } else if x >= scale_for_dpi(EDITOR_PANEL_LEFT_DIP + 16, dpi)
                    && x < scale_for_dpi(EDITOR_WIDTH_DIP - 16, dpi)
                    && y >= scale_for_dpi(116, dpi)
                    && y < scale_for_dpi(438, dpi)
                {
                    focus_editor_control(load_window(&EDITOR_BODY_EDIT));
                } else if y >= scale_for_dpi(14, dpi)
                    && y < scale_for_dpi(46, dpi)
                    && x >= scale_for_dpi(312, dpi)
                    && x < scale_for_dpi(344, dpi)
                {
                    close_note_editor(window);
                } else if y >= scale_for_dpi(14, dpi)
                    && y < scale_for_dpi(46, dpi)
                    && x >= scale_for_dpi(275, dpi)
                    && x < scale_for_dpi(307, dpi)
                {
                    hide_note_editor(window);
                }
                LRESULT(0)
            }
            WM_CLOSE => {
                close_note_editor(window);
                LRESULT(0)
            }
            WM_DESTROY => {
                EDITOR_REVEAL.with(|slot| *slot.borrow_mut() = None);
                EDITOR_WINDOW.store(0, Ordering::Release);
                EDITOR_TITLE_EDIT.store(0, Ordering::Release);
                EDITOR_BODY_EDIT.store(0, Ordering::Release);
                EDITOR_PLACEHOLDER.store(0, Ordering::Release);
                EDITOR_TITLE_PLACEHOLDER.store(0, Ordering::Release);
                delete_editor_font(&EDITOR_TITLE_FONT);
                delete_editor_font(&EDITOR_BODY_FONT);
                EDITOR_NATIVE_CORNERS.store(false, Ordering::Release);
                EDITOR_RENDERER.with(|renderer| *renderer.borrow_mut() = None);
                LRESULT(0)
            }
            _ => DefWindowProcW(window, message, wparam, lparam),
        }
    }
}

unsafe fn open_note_editor(list_window: HWND, note: &NoteSummary) {
    // SAFETY: The editor and its controls are created on the current UI thread.
    unsafe {
        EDITOR_MINIMIZED_TO_LIST.store(false, Ordering::Release);
        set_list_reveal_target(1.0);
        let mut editor = load_window(&EDITOR_WINDOW);
        let was_visible = !editor.0.is_null() && IsWindowVisible(editor).as_bool();
        if !editor.0.is_null() {
            finish_editor_reveal(editor, false);
            save_active_note();
        }
        if let Some(active_id) = ACTIVE_NOTE_ID.get()
            && let Ok(mut active_id) = active_id.lock()
        {
            *active_id = Some(note.id.clone());
        }
        if let Some(active_color) = ACTIVE_NOTE_COLOR.get()
            && let Ok(mut active_color) = active_color.lock()
        {
            note.color.clone_into(&mut active_color);
        }

        let mut list_bounds = RECT::default();
        if GetWindowRect(list_window, &mut list_bounds).is_err() {
            return;
        }
        let center = POINT {
            x: (list_bounds.left + list_bounds.right) / 2,
            y: (list_bounds.top + list_bounds.bottom) / 2,
        };
        let Ok(work_area) = work_area_at(center) else {
            return;
        };
        let dpi = GetDpiForWindow(list_window);
        let width = scale_for_dpi(EDITOR_WIDTH_DIP, dpi);
        let height = scale_for_dpi(EDITOR_HEIGHT_DIP, dpi);
        let gap = scale_for_dpi(EDITOR_GAP_DIP, dpi);
        let x = (list_bounds.left - width - gap).max(work_area.left);
        let y = (center.y - height / 2).clamp(work_area.top, work_area.bottom - height);

        if editor.0.is_null() {
            let instance = HINSTANCE(match GetModuleHandleW(None) {
                Ok(module) => module.0,
                Err(_) => return,
            });
            let class_name = utf16("NotyEditorWindow");
            let title = utf16("Noty 编辑器");
            let Ok(created) = CreateWindowExW(
                WINDOW_EX_STYLE(WS_EX_TOPMOST.0 | WS_EX_TOOLWINDOW.0),
                PCWSTR(class_name.as_ptr()),
                PCWSTR(title.as_ptr()),
                WS_POPUP | WS_CAPTION | WS_THICKFRAME | WS_CLIPCHILDREN,
                x,
                y,
                width,
                height,
                None,
                None,
                Some(instance),
                None,
            ) else {
                return;
            };
            editor = created;
            EDITOR_WINDOW.store(editor.0 as isize, Ordering::Release);
            if let Err(error) = create_editor_controls(editor, instance, GetDpiForWindow(editor)) {
                eprintln!("Failed to create editor controls: {error}");
                let _ = DestroyWindow(editor);
                return;
            }
        } else {
            let _ = SetWindowPos(editor, None, x, y, width, height, SWP_NOZORDER);
        }
        if let Err(error) = update_editor_layout(editor, GetDpiForWindow(editor)) {
            eprintln!("Failed to update editor layout: {error}");
            return;
        }

        load_note_into_editor(note);
        let reveal = if was_visible {
            Ok(false)
        } else {
            client_animations_enabled()
        };
        match reveal {
            Ok(true) => {
                if let Err(error) = begin_editor_reveal(editor, POINT { x, y }, dpi) {
                    eprintln!("Failed to animate editor opening: {error}");
                    finish_editor_reveal(editor, true);
                    let _ = ShowWindow(editor, SW_SHOW);
                }
            }
            Ok(false) => {
                let _ = ShowWindow(editor, SW_SHOW);
            }
            Err(error) => {
                eprintln!("Failed to read Windows animation preference: {error}");
                let _ = ShowWindow(editor, SW_SHOW);
            }
        }
        let _ = SetForegroundWindow(editor);
        let _ = SetFocus(Some(editor));
        let _ = InvalidateRect(Some(editor), None, true);
    }
}

unsafe fn create_editor_controls(window: HWND, instance: HINSTANCE, dpi: u32) -> WinResult<()> {
    // SAFETY: Child controls use standard Windows classes and live under `window`.
    unsafe {
        let edit_class = utf16("EDIT");
        let static_class = utf16("STATIC");
        let empty = utf16("");
        let title = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(edit_class.as_ptr()),
            PCWSTR(empty.as_ptr()),
            WINDOW_STYLE(
                WS_CHILD.0
                    | WS_VISIBLE.0
                    | WS_TABSTOP.0
                    | ES_AUTOHSCROLL as u32
                    | ES_NOHIDESEL as u32,
            ),
            scale_for_dpi(EDITOR_PANEL_LEFT_DIP + 16, dpi),
            scale_for_dpi(64, dpi),
            scale_for_dpi(298, dpi),
            scale_for_dpi(33, dpi),
            Some(window),
            Some(HMENU(EDITOR_TITLE_ID as *mut c_void)),
            Some(instance),
            None,
        )?;
        let body = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(edit_class.as_ptr()),
            PCWSTR(empty.as_ptr()),
            WINDOW_STYLE(
                WS_CHILD.0
                    | WS_VISIBLE.0
                    | WS_TABSTOP.0
                    | ES_MULTILINE as u32
                    | ES_AUTOVSCROLL as u32
                    | ES_WANTRETURN as u32
                    | ES_NOHIDESEL as u32,
            ),
            scale_for_dpi(EDITOR_PANEL_LEFT_DIP + 16, dpi),
            scale_for_dpi(116, dpi),
            scale_for_dpi(298, dpi),
            scale_for_dpi(322, dpi),
            Some(window),
            Some(HMENU(EDITOR_BODY_ID as *mut c_void)),
            Some(instance),
            None,
        )?;
        let placeholder_text =
            utf16("在这里，随手记一点。\r\n\r\n一个灵感、一件待办，\r\n或是不想忘记的小事。");
        let placeholder = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(static_class.as_ptr()),
            PCWSTR(placeholder_text.as_ptr()),
            WINDOW_STYLE(WS_CHILD.0 | 0x0100),
            scale_for_dpi(EDITOR_PANEL_LEFT_DIP + 16, dpi),
            scale_for_dpi(117, dpi),
            scale_for_dpi(285, dpi),
            scale_for_dpi(92, dpi),
            Some(window),
            Some(HMENU(EDITOR_PLACEHOLDER_ID as *mut c_void)),
            Some(instance),
            None,
        )?;
        let title_placeholder_text = utf16("给想法起个名字");
        let title_placeholder = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(static_class.as_ptr()),
            PCWSTR(title_placeholder_text.as_ptr()),
            WINDOW_STYLE(WS_CHILD.0 | 0x0100),
            scale_for_dpi(EDITOR_PANEL_LEFT_DIP + 16, dpi),
            scale_for_dpi(64, dpi),
            scale_for_dpi(298, dpi),
            scale_for_dpi(33, dpi),
            Some(window),
            Some(HMENU(EDITOR_TITLE_PLACEHOLDER_ID as *mut c_void)),
            Some(instance),
            None,
        )?;

        EDITOR_TITLE_EDIT.store(title.0 as isize, Ordering::Release);
        EDITOR_BODY_EDIT.store(body.0 as isize, Ordering::Release);
        EDITOR_PLACEHOLDER.store(placeholder.0 as isize, Ordering::Release);
        EDITOR_TITLE_PLACEHOLDER.store(title_placeholder.0 as isize, Ordering::Release);

        Ok(())
    }
}

fn client_animations_enabled() -> WinResult<bool> {
    let mut enabled = BOOL(0);
    // SAFETY: Windows writes one BOOL into the provided animation-preference buffer.
    unsafe {
        SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            Some(ptr::from_mut(&mut enabled).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )?;
    }
    Ok(enabled.as_bool())
}

fn set_animation_clock_active(active: bool) {
    ANIMATION_CLOCK.with(|slot| {
        if let Some(clock) = slot.borrow().as_ref() {
            clock.set_active(active);
        }
    });
}

unsafe fn begin_editor_reveal(window: HWND, target: POINT, dpi: u32) -> WinResult<()> {
    // SAFETY: Constant window opacity preserves native input controls; no text bitmap is scaled.
    unsafe {
        let original_style = GetWindowLongPtrW(window, GWL_EXSTYLE);
        EDITOR_REVEAL.with(|slot| {
            *slot.borrow_mut() = Some(EditorReveal {
                started: Instant::now(),
                target,
                offset: scale_for_dpi(EDITOR_REVEAL_OFFSET_DIP, dpi),
                original_style,
            });
        });
        if SetWindowLongPtrW(
            window,
            GWL_EXSTYLE,
            original_style | WS_EX_LAYERED.0 as isize,
        ) == 0
        {
            return Err(WinError::from_thread());
        }
        SetLayeredWindowAttributes(window, COLORREF(0), 0, LWA_ALPHA)?;
        SetWindowPos(
            window,
            None,
            target.x + scale_for_dpi(EDITOR_REVEAL_OFFSET_DIP, dpi),
            target.y,
            0,
            0,
            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        )?;
        let _ = ShowWindow(window, SW_SHOWNOACTIVATE);
        if !RedrawWindow(
            Some(window),
            None,
            None,
            RDW_INVALIDATE | RDW_UPDATENOW | RDW_ALLCHILDREN,
        )
        .as_bool()
        {
            return Err(WinError::from_thread());
        }
        EDITOR_REVEAL.with(|slot| {
            if let Some(reveal) = slot.borrow_mut().as_mut() {
                // First-paint/font setup time must not consume animation frames.
                reveal.started = Instant::now();
            }
        });
        set_animation_clock_active(true);
        Ok(())
    }
}

unsafe fn advance_editor_reveal() {
    let frame = EDITOR_REVEAL.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|reveal| reveal.frame(reveal.started.elapsed()))
    });
    let Some((position, alpha, finished)) = frame else {
        return;
    };
    let window = load_window(&EDITOR_WINDOW);
    if window.0.is_null() {
        EDITOR_REVEAL.with(|slot| *slot.borrow_mut() = None);
        return;
    }
    // SAFETY: All native updates occur on the UI thread, after releasing the state borrow.
    unsafe {
        if finished {
            finish_editor_reveal(window, true);
        } else {
            let result = SetWindowPos(
                window,
                None,
                position.x,
                position.y,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            )
            .and_then(|()| SetLayeredWindowAttributes(window, COLORREF(0), alpha, LWA_ALPHA));
            if let Err(error) = result {
                eprintln!("Failed to advance editor opening: {error}");
                finish_editor_reveal(window, true);
            }
        }
    }
}

unsafe fn finish_editor_reveal(window: HWND, snap_to_target: bool) {
    let reveal = EDITOR_REVEAL.with(|slot| slot.borrow_mut().take());
    let Some(reveal) = reveal else { return };
    // SAFETY: State is removed first because window updates may synchronously dispatch messages.
    unsafe {
        if GetWindowLongPtrW(window, GWL_EXSTYLE) & WS_EX_LAYERED.0 as isize != 0 {
            if let Err(error) = SetLayeredWindowAttributes(window, COLORREF(0), 255, LWA_ALPHA) {
                eprintln!("Failed to restore editor opacity: {error}");
            }
        }
        if SetWindowLongPtrW(window, GWL_EXSTYLE, reveal.original_style) == 0 {
            eprintln!(
                "Failed to restore editor window style: {}",
                WinError::from_thread()
            );
        }
        if snap_to_target {
            if let Err(error) = SetWindowPos(
                window,
                None,
                reveal.target.x,
                reveal.target.y,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            ) {
                eprintln!("Failed to settle editor opening: {error}");
            }
        }
        let _ = InvalidateRect(Some(window), None, false);
    }
}

unsafe fn update_editor_layout(window: HWND, dpi: u32) -> WinResult<()> {
    // SAFETY: Only this editor's child windows and owned fonts are updated.
    unsafe {
        let mut client = RECT::default();
        GetClientRect(window, &mut client)?;
        update_editor_frame(window, client, dpi)?;
        for (slot, top, width, height) in [
            (&EDITOR_TITLE_EDIT, 64, 298, 33),
            (&EDITOR_BODY_EDIT, 116, 298, 322),
            (&EDITOR_PLACEHOLDER, 117, 285, 92),
            (&EDITOR_TITLE_PLACEHOLDER, 64, 298, 33),
        ] {
            let control = load_window(slot);
            if !control.0.is_null() {
                SetWindowPos(
                    control,
                    None,
                    scale_for_dpi(EDITOR_PANEL_LEFT_DIP + 16, dpi),
                    scale_for_dpi(top, dpi),
                    scale_for_dpi(width, dpi),
                    scale_for_dpi(height, dpi),
                    SWP_NOZORDER | SWP_NOACTIVATE,
                )?;
            }
        }
        for (slot, height, weight, controls) in [
            (
                &EDITOR_TITLE_FONT,
                20,
                700,
                [&EDITOR_TITLE_EDIT, &EDITOR_TITLE_PLACEHOLDER],
            ),
            (
                &EDITOR_BODY_FONT,
                15,
                NORMAL_FONT_WEIGHT,
                [&EDITOR_BODY_EDIT, &EDITOR_PLACEHOLDER],
            ),
        ] {
            let font = create_editor_font(scale_for_dpi(height, dpi), weight);
            if font.0.is_null() {
                return Err(WinError::from_thread());
            }
            for control in controls {
                let control = load_window(control);
                if !control.0.is_null() {
                    let _ = SendMessageW(
                        control,
                        WM_SETFONT,
                        Some(WPARAM(font.0 as usize)),
                        Some(LPARAM(1)),
                    );
                }
            }
            delete_editor_font(slot);
            slot.store(font.0 as isize, Ordering::Release);
        }
        let _ = InvalidateRect(Some(window), None, false);
        Ok(())
    }
}

unsafe fn update_editor_frame(window: HWND, client: RECT, dpi: u32) -> WinResult<()> {
    // SAFETY: DWM receives pointers to live, correctly sized attribute values.
    unsafe {
        match DwmSetWindowAttribute(
            window,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            ptr::from_ref(&DWMWCP_ROUND).cast(),
            size_of_val(&DWMWCP_ROUND) as u32,
        ) {
            Ok(()) => {
                DwmExtendFrameIntoClientArea(
                    window,
                    &MARGINS {
                        cxLeftWidth: 1,
                        cxRightWidth: 1,
                        cyTopHeight: 1,
                        cyBottomHeight: 1,
                    },
                )?;
                let border = rgb(
                    EDITOR_BORDER_COLOR.red,
                    EDITOR_BORDER_COLOR.green,
                    EDITOR_BORDER_COLOR.blue,
                );
                DwmSetWindowAttribute(
                    window,
                    DWMWA_BORDER_COLOR,
                    (&border as *const COLORREF).cast(),
                    size_of_val(&border) as u32,
                )?;
                EDITOR_NATIVE_CORNERS.store(true, Ordering::Release);
            }
            Err(error) if error.code() == E_INVALIDARG => {
                // Windows 10 has no DWM corner preference; retain its region-based shape.
                eprintln!(
                    "DWM rounded corners are unavailable; using the Windows 10 window region."
                );
                EDITOR_NATIVE_CORNERS.store(false, Ordering::Release);
                let region = CreateRoundRectRgn(
                    0,
                    0,
                    client.right + 1,
                    client.bottom + 1,
                    scale_for_dpi(34, dpi),
                    scale_for_dpi(34, dpi),
                );
                if region.0.is_null() {
                    return Err(WinError::from_thread());
                }
                if SetWindowRgn(window, Some(region), true) == 0 {
                    let error = WinError::from_thread();
                    let _ = DeleteObject(HGDIOBJ(region.0));
                    return Err(error);
                }
            }
            Err(error) => return Err(error),
        }
        Ok(())
    }
}

unsafe fn handle_editor_command(window: HWND, wparam: WPARAM) {
    let control_id = wparam.0 & 0xffff;
    let notification = (wparam.0 >> 16) & 0xffff;
    let body = load_window(&EDITOR_BODY_EDIT);
    let title_edit = load_window(&EDITOR_TITLE_EDIT);

    // SAFETY: All referenced child windows belong to this editor window.
    unsafe {
        if (control_id == EDITOR_TITLE_ID || control_id == EDITOR_BODY_ID)
            && notification == EN_SETFOCUS as usize
        {
            finish_editor_reveal(window, false);
        }
        if (control_id == EDITOR_TITLE_PLACEHOLDER_ID || control_id == EDITOR_PLACEHOLDER_ID)
            && notification == 0
        {
            focus_editor_control(if control_id == EDITOR_TITLE_PLACEHOLDER_ID {
                title_edit
            } else {
                body
            });
            return;
        }
        if control_id == EDITOR_TITLE_ID
            && (notification == EN_SETFOCUS as usize || notification == EN_KILLFOCUS as usize)
        {
            update_title_placeholder();
        }
        if control_id == EDITOR_BODY_ID
            && (notification == EN_SETFOCUS as usize || notification == EN_KILLFOCUS as usize)
        {
            update_body_placeholder();
        }

        if (control_id == EDITOR_TITLE_ID || control_id == EDITOR_BODY_ID)
            && notification == EN_CHANGE as usize
            && !EDITOR_LOADING.load(Ordering::Relaxed)
        {
            EDITOR_DIRTY.store(true, Ordering::Relaxed);
            let _ = SetTimer(Some(window), EDITOR_SAVE_TIMER_ID, 350, None);
            if control_id == EDITOR_BODY_ID {
                update_body_placeholder();
            } else {
                update_title_placeholder();
            }
            invalidate_editor_status(window);
        }
    }
}

unsafe fn load_note_into_editor(note: &NoteSummary) {
    EDITOR_LOADING.store(true, Ordering::Relaxed);
    // SAFETY: The editor controls are valid while the editor window exists.
    unsafe {
        let title = if note.title == "新便签" {
            ""
        } else {
            &note.title
        };
        let title_text = utf16(title);
        let body_text = utf16(&note.content);
        let title_edit = load_window(&EDITOR_TITLE_EDIT);
        let body_edit = load_window(&EDITOR_BODY_EDIT);
        let editor = load_window(&EDITOR_WINDOW);
        if !editor.0.is_null() {
            let _ = SetFocus(Some(editor));
        }
        if !title_edit.0.is_null() {
            let _ = SetWindowTextW(title_edit, PCWSTR(title_text.as_ptr()));
        }
        if !body_edit.0.is_null() {
            let _ = SetWindowTextW(body_edit, PCWSTR(body_text.as_ptr()));
        }
        update_body_placeholder();
        update_title_placeholder();
    }
    EDITOR_LOADING.store(false, Ordering::Relaxed);
    EDITOR_DIRTY.store(false, Ordering::Relaxed);
}

unsafe fn save_active_note() {
    if !EDITOR_DIRTY.load(Ordering::Relaxed) {
        return;
    }
    let note_id = ACTIVE_NOTE_ID.get().and_then(|active_id| {
        active_id
            .lock()
            .ok()
            .and_then(|active_id| active_id.clone())
    });
    let Some(note_id) = note_id else {
        return;
    };
    let title_edit = load_window(&EDITOR_TITLE_EDIT);
    let body_edit = load_window(&EDITOR_BODY_EDIT);
    if title_edit.0.is_null() || body_edit.0.is_null() {
        return;
    }
    // SAFETY: Both handles refer to standard edit controls owned by this UI thread.
    let title = unsafe { window_text(title_edit) };
    let content = unsafe { window_text(body_edit) };
    let stored_title = if title.trim().is_empty() {
        "新便签".to_string()
    } else {
        title.trim().to_string()
    };

    let Some(database) = DATABASE.get() else {
        return;
    };
    let Ok(database) = database.lock() else {
        return;
    };
    if let Err(error) = database.update_note(&note_id, &stored_title, &content) {
        eprintln!("Failed to save note: {error}");
        return;
    }
    EDITOR_DIRTY.store(false, Ordering::Relaxed);
    let updated_notes = match database.list_notes() {
        Ok(notes) => notes,
        Err(error) => {
            eprintln!("Failed to refresh notes after saving: {error}");
            return;
        }
    };
    drop(database);
    let previous_notes = current_notes();
    let order_changed = !previous_notes
        .iter()
        .map(|note| &note.id)
        .eq(updated_notes.iter().map(|note| &note.id));
    let appearance_changed = note_keys(&previous_notes) != note_keys(&updated_notes);
    if let Some(notes) = NOTES.get()
        && let Ok(mut notes) = notes.lock()
    {
        *notes = updated_notes.clone();
    }
    if order_changed {
        reset_list_scroll_animation();
        reset_card_hover_progress();
    }
    // SAFETY: Refreshes occur on the same UI thread as all three windows.
    unsafe {
        if appearance_changed || order_changed {
            refresh_windows_after_edit(&updated_notes);
        }
        let editor = load_window(&EDITOR_WINDOW);
        if !editor.0.is_null() {
            invalidate_editor_status(editor);
        }
    };
}

unsafe fn refresh_windows_after_edit(notes: &[NoteSummary]) {
    // SAFETY: Window handles are checked before rendering into them.
    unsafe {
        let list = load_window(&LIST_WINDOW);
        if !list.0.is_null() && IsWindowVisible(list).as_bool() {
            let mut bounds = RECT::default();
            if GetWindowRect(list, &mut bounds).is_ok() {
                let viewport = note_viewport(notes, GetDpiForWindow(list));
                let _ = render_note_list(
                    list,
                    bounds.right - bounds.left,
                    bounds.bottom - bounds.top,
                    GetDpiForWindow(list),
                    &viewport.notes,
                    notes.len(),
                    list_reveal_alpha(),
                );
            }
        }
        let pill = load_window(&PILL_WINDOW);
        if !pill.0.is_null() {
            let mut bounds = RECT::default();
            if GetWindowRect(pill, &mut bounds).is_ok() {
                let _ = render_pill(
                    pill,
                    bounds.right - bounds.left,
                    bounds.bottom - bounds.top,
                    GetDpiForWindow(pill),
                    notes,
                );
            }
        }
    }
}

unsafe fn close_note_editor(window: HWND) {
    // SAFETY: The editor is closed from its own UI thread.
    unsafe {
        if EDITOR_CLOSING.swap(true, Ordering::AcqRel) {
            return;
        }
        EDITOR_MINIMIZED_TO_LIST.store(false, Ordering::Release);
        let _ = KillTimer(Some(window), EDITOR_SAVE_TIMER_ID);
        save_active_note();
        let _ = DestroyWindow(window);
        if let Some(active_id) = ACTIVE_NOTE_ID.get()
            && let Ok(mut active_id) = active_id.lock()
        {
            *active_id = None;
        }
        let list = load_window(&LIST_WINDOW);
        if !list.0.is_null() {
            hide_note_list(list);
        }
        EDITOR_CLOSING.store(false, Ordering::Release);
    }
}

unsafe fn hide_note_editor(window: HWND) {
    // SAFETY: The editor and list windows belong to the current UI thread.
    unsafe {
        finish_editor_reveal(window, false);
        if EDITOR_CLOSING.swap(true, Ordering::AcqRel) {
            return;
        }
        EDITOR_MINIMIZED_TO_LIST.store(true, Ordering::Release);
        let _ = KillTimer(Some(window), EDITOR_SAVE_TIMER_ID);
        save_active_note();
        let _ = ShowWindow(window, SW_HIDE);
        EDITOR_CLOSING.store(false, Ordering::Release);
    }
}

unsafe fn paint_editor(window: HWND) {
    // SAFETY: BeginPaint and EndPaint bracket all drawing to this window.
    unsafe {
        let mut paint = PAINTSTRUCT::default();
        let dc = BeginPaint(window, &mut paint);
        let mut client = RECT::default();
        let result = GetClientRect(window, &mut client).and_then(|()| {
            if client.right <= 0 || client.bottom <= 0 {
                return Ok(());
            }
            EDITOR_RENDERER.with(|cache| {
                let mut cache = cache.borrow_mut();
                if cache.is_none() {
                    *cache = Some(EditorRenderer::new(client.right, client.bottom)?);
                }
                let renderer = cache.as_mut().expect("editor renderer initialized");
                if renderer.surface.width != client.right
                    || renderer.surface.height != client.bottom
                {
                    renderer.surface = OpaqueSurface::new(client.right, client.bottom)?;
                }
                renderer.render(client, GetDpiForWindow(window))?;
                BitBlt(
                    dc,
                    0,
                    0,
                    client.right,
                    client.bottom,
                    Some(renderer.surface.dc),
                    0,
                    0,
                    SRCCOPY,
                )
            })
        });
        if let Err(error) = result {
            eprintln!("Failed to paint editor: {error}");
            EDITOR_RENDERER.with(|cache| *cache.borrow_mut() = None);
        }
        let _ = EndPaint(window, &paint);
    }
}

unsafe fn paint_editor_background(
    target: &ID2D1DCRenderTarget,
    client: RECT,
    dpi: u32,
) -> WinResult<()> {
    // SAFETY: The caller has begun a drawing pass on the offscreen render target.
    unsafe {
        target.Clear(Some(&d2d_color(Color::new(252, 249, 241))));

        let sidebar_color = EDITOR_SIDEBAR_COLOR;
        let sidebar_brush = target.CreateSolidColorBrush(&d2d_color(sidebar_color), None)?;
        let sidebar = RECT {
            left: 0,
            top: 0,
            right: scale_for_dpi(EDITOR_PANEL_LEFT_DIP, dpi),
            bottom: client.bottom,
        };
        target.FillRectangle(&d2d_rect(sidebar), &sidebar_brush);

        let dash_color = blend_colors(Color::new(75, 71, 59), sidebar_color, 96.0 / 255.0);
        let dash_brush = target.CreateSolidColorBrush(&d2d_color(dash_color), None)?;
        let dash_x = scale_for_dpi(29, dpi);
        let dash_height = scale_for_dpi(5, dpi).max(1);
        let dash_step = scale_for_dpi(10, dpi).max(1);
        let mut dash_y = scale_for_dpi(18, dpi);
        while dash_y < client.bottom - scale_for_dpi(18, dpi) {
            let dash = RECT {
                left: dash_x,
                top: dash_y,
                right: dash_x + scale_for_dpi(1, dpi).max(1),
                bottom: (dash_y + dash_height).min(client.bottom),
            };
            target.FillRectangle(&d2d_rect(dash), &dash_brush);
            dash_y += dash_step;
        }
        let line_brush =
            target.CreateSolidColorBrush(&d2d_color(Color::new(232, 225, 211)), None)?;
        let divider = RECT {
            left: scale_for_dpi(EDITOR_PANEL_LEFT_DIP + 16, dpi),
            top: scale_for_dpi(103, dpi),
            right: client.right - scale_for_dpi(16, dpi),
            bottom: scale_for_dpi(104, dpi),
        };
        target.FillRectangle(&d2d_rect(divider), &line_brush);

        if EDITOR_NATIVE_CORNERS.load(Ordering::Acquire) {
            return Ok(());
        }
        let border_color = darken_color(sidebar_color, 1.05);
        let border_brush = target.CreateSolidColorBrush(&d2d_color(border_color), None)?;
        for border in [
            RECT {
                left: 0,
                top: 0,
                right: client.right,
                bottom: 1,
            },
            RECT {
                left: 0,
                top: client.bottom - 1,
                right: client.right,
                bottom: client.bottom,
            },
            RECT {
                left: 0,
                top: 0,
                right: 1,
                bottom: client.bottom,
            },
            RECT {
                left: client.right - 1,
                top: 0,
                right: client.right,
                bottom: client.bottom,
            },
        ] {
            target.FillRectangle(&d2d_rect(border), &border_brush);
        }
        Ok(())
    }
}

unsafe fn paint_editor_text(renderer: &EditorRenderer, client: RECT, dpi: u32) -> WinResult<()> {
    // SAFETY: Text is drawn at native pixel size within the caller's D2D drawing pass.
    unsafe {
        let width = client.right - client.left;
        let height = client.bottom - client.top;
        if width <= 0 || height <= 0 {
            return Ok(());
        }
        let draw = |text: &str,
                    rect: RECT,
                    style: TextStyle<'_>,
                    alignment: DWRITE_TEXT_ALIGNMENT,
                    paragraph: DWRITE_PARAGRAPH_ALIGNMENT,
                    wrapping: DWRITE_WORD_WRAPPING,
                    grayscale: bool| {
            draw_editor_text(
                renderer, text, rect, style, alignment, paragraph, wrapping, grayscale,
            )
        };

        let brand_style = TextStyle {
            face: UI_FONT_FACE,
            pixel_height: scale_for_dpi(EDITOR_CHROME_FONT_DIP, dpi),
            weight: NORMAL_FONT_WEIGHT,
            color: Color::new(121, 107, 69),
        };
        for (index, character) in ["速", "记", "胶", "囊"].iter().enumerate() {
            let top = scale_for_dpi(22 + index as i32 * 18, dpi);
            draw(
                character,
                RECT {
                    left: 0,
                    top,
                    right: scale_for_dpi(29, dpi),
                    bottom: top + scale_for_dpi(18, dpi),
                },
                brand_style,
                DWRITE_TEXT_ALIGNMENT_CENTER,
                DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                DWRITE_WORD_WRAPPING_NO_WRAP,
                true,
            )?;
        }

        let body_control = load_window(&EDITOR_BODY_EDIT);
        let body = window_text(body_control);

        let status_style = TextStyle {
            face: UI_FONT_FACE,
            pixel_height: scale_for_dpi(EDITOR_CHROME_FONT_DIP, dpi),
            weight: NORMAL_FONT_WEIGHT,
            color: Color::new(154, 146, 132),
        };
        draw(
            "已保存到本机",
            RECT {
                left: scale_for_dpi(EDITOR_PANEL_LEFT_DIP + 27, dpi),
                top: height - scale_for_dpi(32, dpi),
                right: scale_for_dpi(190, dpi),
                bottom: height - scale_for_dpi(7, dpi),
            },
            status_style,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            DWRITE_WORD_WRAPPING_NO_WRAP,
            true,
        )?;
        let count_text = format!("{} 字符", body.chars().count());
        draw(
            &count_text,
            RECT {
                left: width - scale_for_dpi(90, dpi),
                top: height - scale_for_dpi(32, dpi),
                right: width - scale_for_dpi(14, dpi),
                bottom: height - scale_for_dpi(7, dpi),
            },
            status_style,
            DWRITE_TEXT_ALIGNMENT_TRAILING,
            DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            DWRITE_WORD_WRAPPING_NO_WRAP,
            true,
        )?;
        Ok(())
    }
}

unsafe fn draw_editor_text(
    renderer: &EditorRenderer,
    text: &str,
    rect: RECT,
    style: TextStyle<'_>,
    alignment: DWRITE_TEXT_ALIGNMENT,
    paragraph: DWRITE_PARAGRAPH_ALIGNMENT,
    wrapping: DWRITE_WORD_WRAPPING,
    grayscale: bool,
) -> WinResult<()> {
    // SAFETY: The caller brackets these draws with BeginDraw and EndDraw.
    unsafe {
        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;
        if width <= 0 || height <= 0 {
            return Ok(());
        }
        let face = utf16(style.face);
        let locale = utf16("zh-CN");
        let text_format = renderer.rasterizer.write_factory.CreateTextFormat(
            PCWSTR(face.as_ptr()),
            None,
            DWRITE_FONT_WEIGHT(style.weight),
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            style.pixel_height as f32,
            PCWSTR(locale.as_ptr()),
        )?;
        text_format.SetTextAlignment(alignment)?;
        text_format.SetParagraphAlignment(paragraph)?;
        text_format.SetWordWrapping(wrapping)?;
        let trimming = DWRITE_TRIMMING {
            granularity: DWRITE_TRIMMING_GRANULARITY_CHARACTER,
            delimiter: 0,
            delimiterCount: 0,
        };
        let ellipsis = renderer
            .rasterizer
            .write_factory
            .CreateEllipsisTrimmingSign(&text_format)?;
        text_format.SetTrimming(&trimming, &ellipsis)?;

        let render_target = &renderer.target;
        render_target.SetTextAntialiasMode(if grayscale {
            D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE
        } else {
            D2D1_TEXT_ANTIALIAS_MODE_CLEARTYPE
        });
        render_target.SetTextRenderingParams(if grayscale {
            &renderer.rasterizer.rendering_params
        } else {
            &renderer.rasterizer.cleartype_rendering_params
        });
        let text_color = d2d_color(style.color);
        let brush = render_target.CreateSolidColorBrush(&text_color, None)?;
        let layout = D2D_RECT_F {
            left: rect.left as f32,
            top: rect.top as f32,
            right: rect.right as f32,
            bottom: rect.bottom as f32,
        };
        let utf16_text = text.encode_utf16().collect::<Vec<_>>();
        render_target.DrawText(
            &utf16_text,
            &text_format,
            &layout,
            &brush,
            D2D1_DRAW_TEXT_OPTIONS_CLIP,
            DWRITE_MEASURING_MODE_NATURAL,
        );
        Ok(())
    }
}

fn d2d_color(color: Color) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: color.red as f32 / 255.0,
        g: color.green as f32 / 255.0,
        b: color.blue as f32 / 255.0,
        a: 1.0,
    }
}

fn d2d_rect(rect: RECT) -> D2D_RECT_F {
    D2D_RECT_F {
        left: rect.left as f32,
        top: rect.top as f32,
        right: rect.right as f32,
        bottom: rect.bottom as f32,
    }
}

unsafe fn paint_editor_symbols(
    target: &ID2D1DCRenderTarget,
    client: RECT,
    dpi: u32,
) -> WinResult<()> {
    // SAFETY: The caller owns the render target and has begun its drawing pass.
    unsafe {
        let scale = dpi as f32 / 96.0;
        let sidebar = active_editor_color();
        for (radius, color) in [
            (6.0, darken_color(sidebar, 1.2)),
            (5.0, lighten_color(sidebar, 1.03)),
            (4.0, darken_color(sidebar, 1.35)),
        ] {
            let brush = target.CreateSolidColorBrush(&d2d_color(color), None)?;
            target.FillEllipse(
                &D2D1_ELLIPSE {
                    point: Vector2 {
                        X: 254.0 * scale,
                        Y: 30.0 * scale,
                    },
                    radiusX: radius * scale,
                    radiusY: radius * scale,
                },
                &brush,
            );
        }
        let brush = target.CreateSolidColorBrush(&d2d_color(Color::new(117, 145, 99)), None)?;
        target.FillEllipse(
            &D2D1_ELLIPSE {
                point: Vector2 {
                    X: (EDITOR_PANEL_LEFT_DIP as f32 + 18.5) * scale,
                    Y: client.bottom as f32 - 19.5 * scale,
                },
                radiusX: 2.5 * scale,
                radiusY: 2.5 * scale,
            },
            &brush,
        );
        let brush = target.CreateSolidColorBrush(&d2d_color(Color::new(74, 76, 70)), None)?;
        let stroke = scale_for_dpi(1, dpi).max(1) as f32;
        let center_y = snap_stroke_center(30.0 * scale, stroke);
        let half_size = scale_for_dpi(5, dpi) as f32;
        let minimize_x = snap_stroke_center(291.0 * scale, stroke);
        target.DrawLine(
            Vector2 {
                X: minimize_x - half_size,
                Y: center_y,
            },
            Vector2 {
                X: minimize_x + half_size,
                Y: center_y,
            },
            &brush,
            stroke,
            None,
        );
        let close_x = snap_stroke_center(328.0 * scale, stroke);
        for direction in [-1.0, 1.0] {
            target.DrawLine(
                Vector2 {
                    X: close_x - half_size,
                    Y: center_y - half_size * direction,
                },
                Vector2 {
                    X: close_x + half_size,
                    Y: center_y + half_size * direction,
                },
                &brush,
                stroke,
                None,
            );
        }
        Ok(())
    }
}

fn snap_stroke_center(position: f32, stroke: f32) -> f32 {
    (position - stroke / 2.0).round() + stroke / 2.0
}

unsafe fn update_body_placeholder() {
    // SAFETY: Both handles are child windows owned by this UI thread.
    unsafe {
        update_editor_placeholder(
            load_window(&EDITOR_BODY_EDIT),
            load_window(&EDITOR_PLACEHOLDER),
        );
    }
}

unsafe fn update_title_placeholder() {
    // SAFETY: Both handles are child windows owned by this UI thread.
    unsafe {
        update_editor_placeholder(
            load_window(&EDITOR_TITLE_EDIT),
            load_window(&EDITOR_TITLE_PLACEHOLDER),
        );
    }
}

unsafe fn update_editor_placeholder(control: HWND, placeholder: HWND) {
    if control.0.is_null() || placeholder.0.is_null() {
        return;
    }
    // SAFETY: Both handles are live child controls of the editor.
    unsafe {
        let show = GetWindowTextLengthW(control) == 0 && GetFocus() != control;
        if show != IsWindowVisible(placeholder).as_bool() {
            let _ = ShowWindow(placeholder, if show { SW_SHOWNOACTIVATE } else { SW_HIDE });
            if !show {
                let _ = InvalidateRect(Some(control), None, false);
            }
        }
    }
}

unsafe fn invalidate_editor_status(window: HWND) {
    // SAFETY: The status strip excludes both input controls.
    unsafe {
        let mut rect = RECT::default();
        if GetClientRect(window, &mut rect).is_ok() {
            rect.top = rect.bottom - scale_for_dpi(36, GetDpiForWindow(window));
            let _ = InvalidateRect(Some(window), Some(&rect), false);
        }
    }
}

unsafe fn focus_editor_control(control: HWND) {
    // SAFETY: Input controls remain visible for the editor's entire lifetime.
    unsafe {
        if !control.0.is_null() {
            let _ = SetFocus(Some(control));
        }
    }
}

unsafe fn window_text(window: HWND) -> String {
    if window.0.is_null() {
        return String::new();
    }
    // SAFETY: The buffer is sized from the control's reported UTF-16 length.
    unsafe {
        let length = GetWindowTextLengthW(window).max(0) as usize;
        let mut buffer = vec![0_u16; length + 1];
        let copied = GetWindowTextW(window, &mut buffer).max(0) as usize;
        String::from_utf16_lossy(&buffer[..copied])
    }
}

unsafe fn create_editor_font(height: i32, weight: i32) -> HFONT {
    let face = utf16(UI_FONT_FACE);
    // SAFETY: The face-name pointer remains valid for the duration of CreateFontW.
    unsafe {
        CreateFontW(
            -height,
            0,
            0,
            0,
            weight,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            DEFAULT_PITCH.0 as u32 | FF_DONTCARE.0 as u32,
            PCWSTR(face.as_ptr()),
        )
    }
}

fn delete_editor_font(storage: &AtomicIsize) {
    let font = storage.swap(0, Ordering::AcqRel);
    if font != 0 {
        // SAFETY: The font was created by CreateFontW and is no longer selected after destruction.
        unsafe {
            let _ = DeleteObject(HGDIOBJ(font as *mut c_void));
        }
    }
}

fn editor_is_visible() -> bool {
    let editor = load_window(&EDITOR_WINDOW);
    !editor.0.is_null() && unsafe { IsWindowVisible(editor).as_bool() }
}

unsafe fn cursor_inside_editor_group(editor: HWND) -> bool {
    // SAFETY: Window handles are checked by GetWindowRect before their bounds are used.
    unsafe {
        let mut cursor = POINT::default();
        if GetCursorPos(&mut cursor).is_err() {
            return false;
        }
        cursor_inside_window(cursor, editor)
            || cursor_inside_window(cursor, load_window(&LIST_WINDOW))
    }
}

unsafe fn cursor_inside_window(cursor: POINT, window: HWND) -> bool {
    if window.0.is_null() {
        return false;
    }
    let mut bounds = RECT::default();
    // SAFETY: GetWindowRect validates the supplied HWND.
    unsafe { GetWindowRect(window, &mut bounds).is_ok() && point_inside_rect(cursor, bounds) }
}

fn point_inside_rect(point: POINT, rect: RECT) -> bool {
    point.x >= rect.left && point.x < rect.right && point.y >= rect.top && point.y < rect.bottom
}

unsafe fn cursor_inside_minimized_editor_bridge(cursor: POINT, list_bounds: RECT) -> bool {
    if !EDITOR_MINIMIZED_TO_LIST.load(Ordering::Acquire) {
        return false;
    }
    let editor = load_window(&EDITOR_WINDOW);
    if editor.0.is_null() {
        return false;
    }
    let mut editor_bounds = RECT::default();
    // SAFETY: The hidden editor HWND remains live until it is explicitly closed.
    if unsafe { GetWindowRect(editor, &mut editor_bounds) }.is_err() {
        return false;
    }
    point_inside_rect(
        cursor,
        RECT {
            left: editor_bounds.left.min(list_bounds.left),
            top: editor_bounds.top.min(list_bounds.top),
            right: editor_bounds.right.max(list_bounds.right),
            bottom: editor_bounds.bottom.max(list_bounds.bottom),
        },
    )
}

const fn rgb(red: u8, green: u8, blue: u8) -> COLORREF {
    COLORREF(red as u32 | ((green as u32) << 8) | ((blue as u32) << 16))
}

fn darken_color(color: Color, factor: f32) -> Color {
    Color::new(
        (color.red as f32 / factor).round() as u8,
        (color.green as f32 / factor).round() as u8,
        (color.blue as f32 / factor).round() as u8,
    )
}

fn lighten_color(color: Color, factor: f32) -> Color {
    Color::new(
        (color.red as f32 * factor).round().min(255.0) as u8,
        (color.green as f32 * factor).round().min(255.0) as u8,
        (color.blue as f32 * factor).round().min(255.0) as u8,
    )
}

fn blend_colors(foreground: Color, background: Color, opacity: f32) -> Color {
    let opacity = opacity.clamp(0.0, 1.0);
    let mix = |front: u8, back: u8| {
        (front as f32 * opacity + back as f32 * (1.0 - opacity)).round() as u8
    };
    Color::new(
        mix(foreground.red, background.red),
        mix(foreground.green, background.green),
        mix(foreground.blue, background.blue),
    )
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
        let viewport = note_viewport(&notes, dpi);
        let height = list_height(notes.len(), dpi);
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
            &viewport.notes,
            notes.len(),
            0,
        );
        reset_animation_clock();
        set_animation_clock_active(true);
        let _ = ShowWindow(pill_window, SW_HIDE);
    }
}

unsafe fn hide_note_list(_list_window: HWND) {
    set_list_reveal_target(0.0);
    set_animation_clock_active(true);
}

unsafe fn finish_hiding_note_list(list_window: HWND) {
    // SAFETY: Both windows belong to the current UI thread.
    unsafe {
        set_animation_clock_active(false);
        let _ = ShowWindow(list_window, SW_HIDE);
        EDITOR_MINIMIZED_TO_LIST.store(false, Ordering::Release);
        reset_list_scroll_animation();
        reset_card_hover_progress();
        reset_animation_clock();
        let pill_window = load_window(&PILL_WINDOW);
        if !pill_window.0.is_null() {
            let _ = ShowWindow(pill_window, SW_SHOWNOACTIVATE);
        }
    }
}

unsafe fn update_note_list_interaction(window: HWND) {
    // SAFETY: Frame notifications are dispatched on the owning UI thread.
    unsafe {
        let mut cursor = POINT::default();
        let mut bounds = RECT::default();
        if GetCursorPos(&mut cursor).is_err() || GetWindowRect(window, &mut bounds).is_err() {
            return;
        }
        let cursor_inside = point_inside_rect(cursor, bounds);
        if cursor_inside {
            EDITOR_MINIMIZED_TO_LIST.store(false, Ordering::Release);
        }
        let inside_minimized_bridge = cursor_inside_minimized_editor_bridge(cursor, bounds);
        if !cursor_inside
            && !editor_is_visible()
            && !inside_minimized_bridge
            && !LIST_CONTEXT_MENU_ACTIVE.load(Ordering::Acquire)
        {
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
        if frame.changed {
            let _ = SetWindowPos(
                window,
                None,
                x,
                frame.final_y,
                frame.width,
                frame.height,
                SWP_NOACTIVATE | SWP_NOZORDER,
            );
        }

        let all_notes = current_notes();
        let viewport = note_viewport(&all_notes, frame.dpi);
        let notes = &viewport.notes;
        let hovered = if !cursor_inside || frame.target == 0.0 {
            None
        } else {
            let local_x = cursor.x - x;
            let local_y = cursor.y - frame.final_y;
            let progress = card_hover_progress(notes.len());
            (0..notes.len()).rev().find(|index| {
                let top = scale_for_dpi(39 + *index as i32 * 48, frame.dpi)
                    + viewport.offset_y.round() as i32;
                let left = card_left(*index, frame.dpi, &progress);
                local_x >= left
                    && local_x < frame.width
                    && local_y >= top
                    && local_y < top + scale_for_dpi(65, frame.dpi)
                    && local_y < card_viewport_bottom(all_notes.len(), frame.dpi)
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
                notes,
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
        if let Some(note) = note_at_list_point(window, x, y) {
            open_note_editor(window, &note);
            return;
        }

        let dpi = GetDpiForWindow(window);
        let all_notes = current_notes();
        let footer_top = footer_y(all_notes.len(), dpi);
        if x >= scale_for_dpi(15, dpi)
            && x < scale_for_dpi(73, dpi)
            && y >= footer_top
            && y < footer_top + scale_for_dpi(40, dpi)
        {
            add_note();
        }
    }
}

unsafe fn note_at_list_point(window: HWND, x: i32, y: i32) -> Option<NoteSummary> {
    // SAFETY: The list HWND is owned by the current UI thread.
    unsafe {
        let dpi = GetDpiForWindow(window);
        let all_notes = current_notes();
        let viewport = note_viewport(&all_notes, dpi);
        let progress = card_hover_progress(viewport.notes.len());
        (0..viewport.notes.len())
            .rev()
            .find(|index| {
                let top =
                    scale_for_dpi(39 + *index as i32 * 48, dpi) + viewport.offset_y.round() as i32;
                let left = card_left(*index, dpi, &progress);
                x >= left
                    && y >= top
                    && y < top + scale_for_dpi(65, dpi)
                    && y < card_viewport_bottom(all_notes.len(), dpi)
            })
            .map(|index| viewport.notes[index].clone())
    }
}

struct ListContextMenuGuard;

impl ListContextMenuGuard {
    fn new() -> Self {
        LIST_CONTEXT_MENU_ACTIVE.store(true, Ordering::Release);
        Self
    }
}

impl Drop for ListContextMenuGuard {
    fn drop(&mut self) {
        LIST_CONTEXT_MENU_ACTIVE.store(false, Ordering::Release);
    }
}

unsafe fn handle_list_context_menu(window: HWND, lparam: LPARAM) {
    let x = lparam.0 as i16 as i32;
    let y = (lparam.0 >> 16) as i16 as i32;
    // SAFETY: Menu creation, tracking, and note mutations all run on the list UI thread.
    unsafe {
        let Some(note) = note_at_list_point(window, x, y) else {
            return;
        };
        let _guard = ListContextMenuGuard::new();
        let Ok(menu) = CreatePopupMenu() else {
            return;
        };
        let pin_label = utf16(if note.is_pinned {
            "取消置顶"
        } else {
            "置顶"
        });
        let delete_label = utf16("删除");
        if AppendMenuW(
            menu,
            MF_STRING,
            NOTE_MENU_TOGGLE_PIN_ID,
            PCWSTR(pin_label.as_ptr()),
        )
        .is_err()
            || AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null()).is_err()
            || AppendMenuW(
                menu,
                MF_STRING,
                NOTE_MENU_DELETE_ID,
                PCWSTR(delete_label.as_ptr()),
            )
            .is_err()
        {
            let _ = DestroyMenu(menu);
            return;
        }

        let mut cursor = POINT::default();
        if GetCursorPos(&mut cursor).is_err() {
            let _ = DestroyMenu(menu);
            return;
        }
        let _ = SetForegroundWindow(window);
        let command = TrackPopupMenu(
            menu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON,
            cursor.x,
            cursor.y,
            None,
            window,
            None,
        )
        .0 as usize;
        let _ = DestroyMenu(menu);

        match command {
            NOTE_MENU_TOGGLE_PIN_ID => set_note_pinned(&note, !note.is_pinned),
            NOTE_MENU_DELETE_ID => confirm_and_delete_note(window, &note),
            _ => {}
        }
    }
}

unsafe fn scroll_note_list(window: HWND, wparam: WPARAM) {
    let delta = ((wparam.0 >> 16) & 0xffff) as u16 as i16 as i32;
    if delta == 0 {
        return;
    }

    let note_count = current_notes().len();
    let dpi = unsafe { GetDpiForWindow(window) };
    update_list_scroll_target(delta, note_count, dpi);
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
    apply_note_collection(updated_notes);
}

fn set_note_pinned(note: &NoteSummary, is_pinned: bool) {
    let Some(database) = DATABASE.get() else {
        return;
    };
    let Ok(database) = database.lock() else {
        return;
    };
    if database.set_note_pinned(&note.id, is_pinned).is_err() {
        return;
    }
    let Ok(updated_notes) = database.list_notes() else {
        return;
    };
    drop(database);
    apply_note_collection(updated_notes);
}

unsafe fn confirm_and_delete_note(window: HWND, note: &NoteSummary) {
    let prompt = utf16(&format!(
        "确定删除“{}”吗？\r\n\r\n删除后无法恢复。",
        note_display_title(note)
    ));
    let caption = utf16("删除便签");
    // SAFETY: Strings and owner HWND remain valid for the modal call.
    let result = unsafe {
        MessageBoxW(
            Some(window),
            PCWSTR(prompt.as_ptr()),
            PCWSTR(caption.as_ptr()),
            MB_YESNO | MB_ICONWARNING | MB_DEFBUTTON2,
        )
    };
    if result != IDYES {
        return;
    }

    let Some(database) = DATABASE.get() else {
        return;
    };
    let Ok(database) = database.lock() else {
        return;
    };
    if database.delete_note(&note.id).is_err() {
        return;
    }
    let Ok(updated_notes) = database.list_notes() else {
        return;
    };
    drop(database);
    // SAFETY: The editor, if it belongs to the deleted note, is owned by this UI thread.
    unsafe { discard_deleted_note_editor(&note.id) };
    apply_note_collection(updated_notes);
}

unsafe fn discard_deleted_note_editor(note_id: &str) {
    let active_matches = ACTIVE_NOTE_ID
        .get()
        .and_then(|active_id| {
            active_id
                .lock()
                .ok()
                .map(|active_id| active_id.as_deref() == Some(note_id))
        })
        .unwrap_or(false);
    if !active_matches {
        return;
    }
    // SAFETY: The editor and timer belong to the current UI thread.
    unsafe {
        let editor = load_window(&EDITOR_WINDOW);
        EDITOR_DIRTY.store(false, Ordering::Release);
        EDITOR_MINIMIZED_TO_LIST.store(false, Ordering::Release);
        if !editor.0.is_null() {
            let _ = KillTimer(Some(editor), EDITOR_SAVE_TIMER_ID);
            let _ = DestroyWindow(editor);
        }
        if let Some(active_id) = ACTIVE_NOTE_ID.get()
            && let Ok(mut active_id) = active_id.lock()
        {
            *active_id = None;
        }
    }
}

fn apply_note_collection(updated_notes: Vec<NoteSummary>) {
    reset_list_scroll_animation();
    reset_card_hover_progress();
    if let Some(cache) = LIST_RENDER_CACHE.get()
        && let Ok(mut cache) = cache.lock()
    {
        *cache = None;
    }

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
                let height = list_height(updated_notes.len(), dpi);
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
                let viewport = note_viewport(&updated_notes, dpi);
                let _ = render_note_list(
                    list_window,
                    bounds.right - bounds.left,
                    bounds.bottom - bounds.top,
                    dpi,
                    &viewport.notes,
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
            let dpi = GetDpiForWindow(window);
            let viewport = note_viewport(&notes, dpi);
            let _ = render_note_list(
                window,
                bounds.right - bounds.left,
                bounds.bottom - bounds.top,
                dpi,
                &viewport.notes,
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
        let can_reuse_base = cache_storage
            .as_ref()
            .is_some_and(|cache| cache.layout_matches(width, height, dpi, total_note_count));
        if can_reuse_base {
            let card_sprites = unsafe { build_card_sprites(width, dpi, notes, None)? };
            let cache = cache_storage
                .as_mut()
                .expect("render cache layout was just matched");
            cache.note_keys = note_keys(notes);
            cache.card_sprites = card_sprites;
        } else {
            // SAFETY: Cache construction uses temporary GDI resources on this UI thread.
            *cache_storage = Some(unsafe {
                build_list_render_cache(width, height, dpi, notes, total_note_count)?
            });
        }
    }

    let cache = cache_storage
        .as_ref()
        .expect("render cache was just created");
    let mut pixels = cache.base_pixels.clone();
    let scroll_offset_y = note_viewport_offset(dpi);
    for (index, variants) in cache.card_sprites.iter().enumerate() {
        let top =
            scale_for_dpi(39 + index as i32 * LIST_SCROLL_STEP_DIP, dpi) as f32 + scroll_offset_y;
        let shift = card_shift_f32(index, dpi, &progress);
        composite_interpolated_card(
            &mut pixels,
            SIZE {
                cx: width,
                cy: height,
            },
            variants.as_slice(),
            shift,
            top,
            scale_for_dpi(39, dpi),
            card_viewport_bottom(total_note_count, dpi),
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
    let mut base_pixels = render_list_base_shapes(width, height, dpi, total_note_count);
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

        let footer_top = footer_y(total_note_count, dpi);
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

    let card_sprites = unsafe { build_card_sprites(width, dpi, notes, Some(&text_rasterizer))? };

    Ok(ListRenderCache {
        width,
        height,
        dpi,
        total_note_count,
        note_keys: note_keys(notes),
        base_pixels,
        card_sprites,
    })
}

unsafe fn build_card_sprites(
    width: i32,
    dpi: u32,
    notes: &[NoteSummary],
    shared_renderer: Option<&TextRasterizer>,
) -> WinResult<Vec<Arc<Vec<CardSprite>>>> {
    let card_height = scale_for_dpi(65, dpi);
    let resting_width = width - scale_for_dpi(17, dpi);
    let mut owned_renderer = None;
    let mut card_sprites = Vec::with_capacity(notes.len());
    for note in notes {
        let display_title = note_display_title(note);
        let key = CardSpriteKey {
            title: display_title.to_string(),
            shape: CardShapeKey {
                color: note.color.clone(),
                resting_width,
                card_height,
                dpi,
            },
        };
        let cached = CARD_SPRITE_CACHE
            .get()
            .and_then(|cache| cache.lock().ok()?.get(&key).cloned());
        if let Some(cached) = cached {
            card_sprites.push(cached);
            continue;
        }

        if shared_renderer.is_none() && owned_renderer.is_none() {
            owned_renderer = Some(unsafe { TextRasterizer::new()? });
        }
        let renderer = shared_renderer
            .or(owned_renderer.as_ref())
            .expect("a text renderer was created for an uncached card");
        let mut text_pixels = vec![0_u8; (resting_width * card_height * 4) as usize];
        unsafe {
            draw_text_layer(
                renderer,
                &mut text_pixels,
                resting_width,
                card_height,
                display_title,
                RECT {
                    left: scale_for_dpi(14, dpi),
                    top: scale_for_dpi(6, dpi),
                    right: resting_width - scale_for_dpi(13, dpi),
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
        let shapes = card_shape_variants(&key.shape);
        let mut variants = Vec::with_capacity(shapes.len());
        for shape in shapes.iter() {
            let mut sprite_pixels = shape.pixels.clone();
            composite_layer(
                &mut sprite_pixels,
                shape.width,
                card_height,
                &text_pixels,
                resting_width,
                card_height,
            );
            variants.push(CardSprite {
                width: shape.width,
                height: card_height,
                pixels: sprite_pixels,
            });
        }
        let variants = Arc::new(variants);
        if let Some(cache) = CARD_SPRITE_CACHE.get()
            && let Ok(mut cache) = cache.lock()
        {
            if cache.len() >= 64 {
                cache.clear();
            }
            cache.insert(key, Arc::clone(&variants));
        }
        card_sprites.push(variants);
    }
    Ok(card_sprites)
}

fn card_shape_variants(key: &CardShapeKey) -> Arc<Vec<CardSprite>> {
    CARD_SHAPE_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(shapes) = cache.get(key) {
            return Arc::clone(shapes);
        }
        let mut shapes = Vec::new();
        let (fill, border) = card_colors(&key.color);
        for shift in 0..=scale_for_dpi(CARD_HOVER_SHIFT_DIP, key.dpi) {
            let width = key.resting_width + shift;
            let mut pixels = vec![0_u8; (width * key.card_height * 4) as usize];
            let card = RECT {
                left: 0,
                top: 0,
                right: width,
                bottom: key.card_height,
            };
            paint_rounded_rect(
                &mut pixels,
                width,
                key.card_height,
                card,
                scale_for_dpi(12, key.dpi) as f32,
                border,
            );
            paint_rounded_rect(
                &mut pixels,
                width,
                key.card_height,
                inset_rect(card, scale_for_dpi(1, key.dpi)),
                scale_for_dpi(11, key.dpi) as f32,
                fill,
            );
            shapes.push(CardSprite {
                width,
                height: key.card_height,
                pixels,
            });
        }
        let shapes = Arc::new(shapes);
        if cache.len() >= 16 {
            cache.clear();
        }
        cache.insert(key.clone(), Arc::clone(&shapes));
        shapes
    })
}

fn note_keys(notes: &[NoteSummary]) -> Vec<(String, String)> {
    notes
        .iter()
        .map(|note| (note.title.clone(), note.color.clone()))
        .collect()
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
    destination_y: f32,
    clip_top: i32,
    clip_bottom: i32,
) {
    let lower = &variants[(shift.floor() as usize).min(variants.len() - 1)];
    let upper = &variants[(shift.ceil() as usize).min(variants.len() - 1)];
    let fraction = shift.fract();
    let left = destination_size.cx - lower.width.max(upper.width);

    let sprite_height = lower.height.max(upper.height);
    let first_target_y = destination_y.floor() as i32;
    let last_target_y = (destination_y + sprite_height as f32).ceil() as i32;
    for target_y in first_target_y..last_target_y {
        if target_y < clip_top || target_y >= clip_bottom || target_y >= destination_size.cy {
            continue;
        }
        let source_y = target_y as f32 - destination_y;
        let source_y_lower = source_y.floor() as i32;
        let source_y_upper = source_y_lower + 1;
        let vertical_fraction = source_y - source_y_lower as f32;
        for target_x in left.max(0)..destination_size.cx {
            let target_offset = ((target_y * destination_size.cx + target_x) * 4) as usize;
            let lower_x = target_x - (destination_size.cx - lower.width);
            let upper_x = target_x - (destination_size.cx - upper.width);
            let mut interpolated = [0_u8; 4];
            for (channel, value) in interpolated.iter_mut().enumerate() {
                let lower_from = sprite_channel(lower, lower_x, source_y_lower, channel) as f32;
                let lower_to = sprite_channel(lower, lower_x, source_y_upper, channel) as f32;
                let from = lower_from + (lower_to - lower_from) * vertical_fraction;
                let upper_from = sprite_channel(upper, upper_x, source_y_lower, channel) as f32;
                let upper_to = sprite_channel(upper, upper_x, source_y_upper, channel) as f32;
                let to = upper_from + (upper_to - upper_from) * vertical_fraction;
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

fn composite_layer(
    destination: &mut [u8],
    destination_width: i32,
    destination_height: i32,
    source: &[u8],
    source_width: i32,
    source_height: i32,
) {
    for y in 0..source_height.min(destination_height) {
        for x in 0..source_width.min(destination_width) {
            let source_offset = ((y * source_width + x) * 4) as usize;
            let alpha = source[source_offset + 3] as u32;
            if alpha == 0 {
                continue;
            }
            let destination_offset = ((y * destination_width + x) * 4) as usize;
            let inverse = 255 - alpha;
            for channel in 0..3 {
                destination[destination_offset + channel] = (source[source_offset + channel] as u32
                    + destination[destination_offset + channel] as u32 * inverse / 255)
                    .min(255) as u8;
            }
            destination[destination_offset + 3] =
                (alpha + destination[destination_offset + 3] as u32 * inverse / 255).min(255) as u8;
        }
    }
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
    // SAFETY: Existing list labels keep their established alignment and truncation.
    unsafe {
        draw_text_layer_formatted(
            renderer,
            pixels,
            width,
            height,
            text,
            rect,
            style,
            DWRITE_TEXT_ALIGNMENT_LEADING,
            DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
            DWRITE_WORD_WRAPPING_NO_WRAP,
        )
    }
}

unsafe fn draw_text_layer_formatted(
    renderer: &TextRasterizer,
    pixels: &mut [u8],
    width: i32,
    height: i32,
    text: &str,
    rect: RECT,
    style: TextStyle<'_>,
    alignment: DWRITE_TEXT_ALIGNMENT,
    paragraph: DWRITE_PARAGRAPH_ALIGNMENT,
    wrapping: DWRITE_WORD_WRAPPING,
) -> WinResult<()> {
    // SAFETY: DirectWrite rasterizes into a temporary D2D DC target. The resulting
    // grayscale coverage is composited into the premultiplied layered-window buffer.
    unsafe {
        let layer_width = rect.right - rect.left;
        let layer_height = rect.bottom - rect.top;
        if layer_width <= 0 || layer_height <= 0 {
            return Ok(());
        }
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
        text_format.SetTextAlignment(alignment)?;
        text_format.SetParagraphAlignment(paragraph)?;
        text_format.SetWordWrapping(wrapping)?;
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
        let bitmap_info = top_down_bitmap_info(layer_width, layer_height);
        let mut bits: *mut c_void = ptr::null_mut();
        let bitmap = CreateDIBSection(
            Some(screen_dc),
            &bitmap_info,
            DIB_RGB_COLORS,
            &mut bits,
            None,
            0,
        )?;
        ptr::write_bytes(
            bits.cast::<u8>(),
            0,
            (layer_width * layer_height * 4) as usize,
        );
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
            right: layer_width,
            bottom: layer_height,
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
            left: 0.0,
            top: 0.0,
            right: layer_width as f32,
            bottom: layer_height as f32,
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

        let mask = std::slice::from_raw_parts(
            bits.cast::<u8>(),
            (layer_width * layer_height * 4) as usize,
        );
        for y in rect.top.max(0)..rect.bottom.min(height) {
            for x in rect.left.max(0)..rect.right.min(width) {
                let offset = (((y - rect.top) * layer_width + x - rect.left) * 4) as usize;
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

fn max_scroll_position(note_count: usize, dpi: u32) -> f32 {
    if note_count <= LIST_VISIBLE_NOTE_LIMIT {
        return 0.0;
    }
    let content_height = note_count as i32 * scale_for_dpi(LIST_SCROLL_STEP_DIP, dpi);
    let viewport_height = scale_for_dpi(258, dpi);
    content_height.saturating_sub(viewport_height) as f32
}

fn note_viewport(notes: &[NoteSummary], dpi: u32) -> NoteViewport {
    let (position, animation_step) = list_scroll_position();
    let step = if animation_step > f32::EPSILON {
        animation_step
    } else {
        scale_for_dpi(LIST_SCROLL_STEP_DIP, dpi) as f32
    };
    let position = position.max(0.0);
    let start = ((position / step).floor() as usize).min(max_scroll_offset(notes.len()));
    let offset_y = -(position - start as f32 * step);
    let end = (start + LIST_VISIBLE_NOTE_LIMIT + 1).min(notes.len());
    NoteViewport {
        notes: notes[start..end].to_vec(),
        offset_y,
    }
}

fn note_viewport_offset(dpi: u32) -> f32 {
    let (position, animation_step) = list_scroll_position();
    let step = if animation_step > f32::EPSILON {
        animation_step
    } else {
        scale_for_dpi(LIST_SCROLL_STEP_DIP, dpi) as f32
    };
    let position = position.max(0.0);
    -(position - (position / step).floor() * step)
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

fn update_list_scroll_target(delta: i32, note_count: usize, dpi: u32) {
    let Some(animation) = LIST_SCROLL_ANIMATION.get() else {
        return;
    };
    let Ok(mut animation) = animation.lock() else {
        return;
    };
    let step = scale_for_dpi(LIST_SCROLL_STEP_DIP, dpi) as f32;
    let max_position = max_scroll_position(note_count, dpi);
    if max_position <= f32::EPSILON {
        return;
    }
    let wheel_distance = -(delta as f32 / 120.0) * step;
    let next_target = (animation.target + wheel_distance).clamp(0.0, max_position);
    if (next_target - animation.target).abs() <= f32::EPSILON {
        return;
    }
    animation.target = next_target;
    animation.step = step;
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
    let delta_seconds = now
        .duration_since(animation.last_tick)
        .as_secs_f32()
        .clamp(0.0, 0.05);
    animation.last_tick = now;
    let old_start = (animation.position / animation.step).floor() as usize;
    let omega = 2.0 / LIST_SCROLL_SMOOTH_TIME_SECONDS;
    let x = omega * delta_seconds;
    let decay = 1.0 / (1.0 + x + 0.48 * x * x + 0.235 * x * x * x);
    let change = animation.position - animation.target;
    let temporary = (animation.velocity + omega * change) * delta_seconds;
    animation.velocity = (animation.velocity - omega * temporary) * decay;
    animation.position = animation.target + (change + temporary) * decay;
    if (animation.position - animation.target).abs() < 0.05 && animation.velocity.abs() < 0.5 {
        animation.position = animation.target;
        animation.velocity = 0.0;
        animation.active = false;
    }
    let new_start = (animation.position / animation.step).floor() as usize;
    drop(animation);
    if old_start != new_start {
        shift_card_hover_progress(old_start, new_start);
    }
    true
}

fn shift_card_hover_progress(old_start: usize, new_start: usize) {
    let Some(progress) = CARD_HOVER_PROGRESS.get() else {
        return;
    };
    let Ok(mut progress) = progress.lock() else {
        return;
    };
    let old = progress.clone();
    let count = old.len();
    let mut shifted = vec![0.0; count];
    if new_start > old_start {
        let steps = (new_start - old_start).min(count);
        shifted[..count - steps].copy_from_slice(&old[steps..]);
    } else {
        let steps = (old_start - new_start).min(count);
        shifted[steps..].copy_from_slice(&old[..count - steps]);
    }
    *progress = shifted;
}

fn list_scroll_position() -> (f32, f32) {
    LIST_SCROLL_ANIMATION
        .get()
        .and_then(|animation| {
            animation
                .lock()
                .ok()
                .map(|animation| (animation.position, animation.step))
        })
        .unwrap_or((0.0, LIST_SCROLL_STEP_DIP as f32))
}

fn reset_list_scroll_animation() {
    let Some(animation) = LIST_SCROLL_ANIMATION.get() else {
        return;
    };
    let Ok(mut animation) = animation.lock() else {
        return;
    };
    animation.position = 0.0;
    animation.target = 0.0;
    animation.velocity = 0.0;
    animation.step = LIST_SCROLL_STEP_DIP as f32;
    animation.active = false;
    animation.last_tick = Instant::now();
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
    let logical_height = (note_count.min(5) as i32 * LIST_SCROLL_STEP_DIP + 114).max(210);
    scale_for_dpi(logical_height, dpi)
}

fn pill_height(note_count: usize, dpi: u32) -> i32 {
    let logical_height =
        (note_count as i32 * 9 + 24).clamp(PILL_MIN_HEIGHT_DIP, PILL_MAX_HEIGHT_DIP);
    scale_for_dpi(logical_height, dpi)
}

fn footer_y(note_count: usize, dpi: u32) -> i32 {
    list_height(note_count, dpi) - scale_for_dpi(48, dpi)
}

fn card_viewport_bottom(note_count: usize, dpi: u32) -> i32 {
    list_height(note_count, dpi) - scale_for_dpi(58, dpi)
}

fn card_colors(color: &str) -> (Color, Color) {
    match color {
        "green" => (Color::new(197, 221, 185), Color::new(166, 199, 150)),
        "yellow" => (Color::new(247, 230, 166), Color::new(225, 204, 127)),
        _ => (Color::new(222, 212, 239), Color::new(194, 175, 221)),
    }
}

fn active_editor_color() -> Color {
    let color = ACTIVE_NOTE_COLOR
        .get()
        .and_then(|color| color.lock().ok().map(|color| color.clone()))
        .unwrap_or_else(|| String::from("purple"));
    card_colors(&color).0
}

fn note_display_title(note: &NoteSummary) -> &str {
    if note.title.trim().is_empty() {
        "新便签"
    } else {
        &note.title
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
        ANIMATION_CLOCK.with(|slot| *slot.borrow_mut() = None);
        let editor = load_window(&EDITOR_WINDOW);
        let pill = load_window(&PILL_WINDOW);
        let list = load_window(&LIST_WINDOW);
        if !editor.0.is_null() {
            save_active_note();
            let _ = DestroyWindow(editor);
        }
        if !pill.0.is_null() {
            let _ = DestroyWindow(pill);
        }
        if !list.0.is_null() {
            let _ = DestroyWindow(list);
        }
        let brush = EDITOR_BACKGROUND_BRUSH.swap(0, Ordering::AcqRel);
        if brush != 0 {
            let _ = DeleteObject(HGDIOBJ(brush as *mut c_void));
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

    static EDITOR_TEST_LOCK: Mutex<()> = Mutex::new(());

    fn surface_pixels(surface: &OpaqueSurface) -> Vec<u8> {
        use windows::Win32::Graphics::Gdi::{BITMAP, GdiFlush, GetObjectW};

        // SAFETY: Rendering is finished and the selected DIB remains alive during the copy.
        unsafe {
            assert!(GdiFlush().as_bool());
            let mut bitmap = BITMAP::default();
            assert_ne!(
                GetObjectW(
                    HGDIOBJ(surface.bitmap.0),
                    size_of::<BITMAP>() as i32,
                    Some((&mut bitmap as *mut BITMAP).cast()),
                ),
                0
            );
            std::slice::from_raw_parts(
                bitmap.bmBits.cast::<u8>(),
                (surface.width * surface.height * 4) as usize,
            )
            .to_vec()
        }
    }

    #[test]
    fn editor_chrome_renders_at_native_dpi_and_reuses_its_surface() {
        let _guard = EDITOR_TEST_LOCK.lock().unwrap();
        // SAFETY: The renderer and its GDI resources stay on this test thread.
        unsafe {
            let mut renderer = EditorRenderer::new(EDITOR_WIDTH_DIP, EDITOR_HEIGHT_DIP).unwrap();
            for dpi in [96, 120, 144, 192, 96] {
                let client = RECT {
                    left: 0,
                    top: 0,
                    right: scale_for_dpi(EDITOR_WIDTH_DIP, dpi),
                    bottom: scale_for_dpi(EDITOR_HEIGHT_DIP, dpi),
                };
                if renderer.surface.width != client.right
                    || renderer.surface.height != client.bottom
                {
                    renderer.surface = OpaqueSurface::new(client.right, client.bottom).unwrap();
                }
                renderer.render(client, dpi).unwrap();
                let first_frame = surface_pixels(&renderer.surface);
                let pixel = |x: i32, y: i32| {
                    let offset = ((y * client.right + x) * 4) as usize;
                    &first_frame[offset..offset + 3]
                };
                assert_eq!(
                    pixel(scale_for_dpi(100, dpi), scale_for_dpi(200, dpi)),
                    [241, 249, 252],
                    "paper must remain opaque at {dpi} DPI"
                );
                let stroke = scale_for_dpi(1, dpi).max(1) as f32;
                let y = snap_stroke_center(30.0 * dpi as f32 / 96.0, stroke) as i32;
                assert_eq!(
                    pixel(scale_for_dpi(291, dpi), y),
                    [70, 76, 74],
                    "minimize stroke must not blur at {dpi} DPI"
                );
                for (left, top, right, bottom) in [
                    (0, 22, 29, 86),
                    (57, 448, 190, 473),
                    (270, 448, 346, 473),
                    (320, 22, 337, 39),
                ] {
                    let mut ink_pixels = 0;
                    for y in scale_for_dpi(top, dpi)..scale_for_dpi(bottom, dpi) {
                        for x in scale_for_dpi(left, dpi)..scale_for_dpi(right, dpi) {
                            if pixel(x, y).iter().all(|channel| *channel < 180) {
                                ink_pixels += 1;
                            }
                        }
                    }
                    assert!(ink_pixels > 15, "missing editor chrome at {dpi} DPI");
                }
                let bitmap = renderer.surface.bitmap;
                renderer.render(client, dpi).unwrap();
                assert_eq!(renderer.surface.bitmap, bitmap);
                assert_eq!(surface_pixels(&renderer.surface), first_frame);
            }
        }
    }

    struct TestEditor(HWND);

    impl Drop for TestEditor {
        fn drop(&mut self) {
            // SAFETY: The fixture owns this window and its background brush.
            unsafe {
                let _ = DestroyWindow(self.0);
                let brush = EDITOR_BACKGROUND_BRUSH.swap(0, Ordering::AcqRel);
                if brush != 0 {
                    let _ = DeleteObject(HGDIOBJ(brush as *mut c_void));
                }
                EDITOR_LOADING.store(false, Ordering::Release);
                EDITOR_DIRTY.store(false, Ordering::Release);
            }
        }
    }

    unsafe extern "system" fn test_editor_proc(
        window: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if message == WM_ACTIVATE {
            return LRESULT(0);
        }
        // SAFETY: This is a normal editor, except focus changes cannot close the test fixture.
        unsafe { editor_window_proc(window, message, wparam, lparam) }
    }

    unsafe fn print_control(control: HWND) -> OpaqueSurface {
        use windows::Win32::UI::WindowsAndMessaging::{PRF_CLIENT, PRF_ERASEBKGND, WM_PRINTCLIENT};

        // SAFETY: WM_PRINTCLIENT draws synchronously into the fixture-owned memory DC.
        unsafe {
            let mut rect = RECT::default();
            GetClientRect(control, &mut rect).unwrap();
            let surface = OpaqueSurface::new(rect.right, rect.bottom).unwrap();
            SendMessageW(
                control,
                WM_PRINTCLIENT,
                Some(WPARAM(surface.dc.0 as usize)),
                Some(LPARAM((PRF_CLIENT | PRF_ERASEBKGND) as isize)),
            );
            surface
        }
    }

    #[test]
    fn editor_text_and_scroll_stay_identical_across_focus_changes() {
        use windows::Win32::UI::Controls::{
            EM_GETFIRSTVISIBLELINE, EM_GETRECT, EM_REPLACESEL, EM_SCROLLCARET, EM_SETSEL,
        };
        use windows::Win32::UI::WindowsAndMessaging::{GWL_STYLE, GetWindowLongW};

        let _guard = EDITOR_TEST_LOCK.lock().unwrap();
        // SAFETY: The test owns all HWNDs and keeps them offscreen on a single thread.
        unsafe {
            let instance = HINSTANCE(GetModuleHandleW(None).unwrap().0);
            let class = utf16("NotyEditorRegressionTest");
            register_editor_window_class(instance, PCWSTR(class.as_ptr()), test_editor_proc)
                .unwrap();
            let window = CreateWindowExW(
                WS_EX_TOOLWINDOW,
                PCWSTR(class.as_ptr()),
                PCWSTR::null(),
                WS_POPUP | WS_CAPTION | WS_THICKFRAME | WS_CLIPCHILDREN,
                -10000,
                -10000,
                EDITOR_WIDTH_DIP,
                EDITOR_HEIGHT_DIP,
                None,
                None,
                Some(instance),
                None,
            )
            .unwrap();
            let editor = TestEditor(window);
            EDITOR_WINDOW.store(window.0 as isize, Ordering::Release);
            EDITOR_BACKGROUND_BRUSH.store(
                CreateSolidBrush(rgb(252, 249, 241)).0 as isize,
                Ordering::Release,
            );
            EDITOR_LOADING.store(true, Ordering::Release);
            create_editor_controls(window, instance, 96).unwrap();
            let _ = ShowWindow(window, SW_SHOWNOACTIVATE);
            let title = load_window(&EDITOR_TITLE_EDIT);
            let body = load_window(&EDITOR_BODY_EDIT);
            for dpi in [96, 120, 144, 192] {
                SetWindowPos(
                    window,
                    None,
                    -10000,
                    -10000,
                    scale_for_dpi(EDITOR_WIDTH_DIP, dpi),
                    scale_for_dpi(EDITOR_HEIGHT_DIP, dpi),
                    SWP_NOZORDER | SWP_NOACTIVATE,
                )
                .unwrap();
                update_editor_layout(window, dpi).unwrap();
                for (control, text) in [
                    (title, "开发任务总结"),
                    (body, "这是一条测试便签。\r\n输入和阅读保持同一套字形。"),
                ] {
                    let _ = SetFocus(Some(window));
                    SetWindowTextW(control, PCWSTR(utf16(text).as_ptr())).unwrap();
                    SendMessageW(control, EM_SETSEL, Some(WPARAM(0)), Some(LPARAM(0)));
                    let before = surface_pixels(&print_control(control));
                    assert!(before.chunks_exact(4).any(|pixel| pixel[0] < 100));
                    let mut rect_before = RECT::default();
                    SendMessageW(
                        control,
                        EM_GETRECT,
                        None,
                        Some(LPARAM(ptr::from_mut(&mut rect_before) as isize)),
                    );
                    let _ = SetFocus(Some(control));
                    assert_eq!(GetFocus(), control);
                    assert_eq!(
                        surface_pixels(&print_control(control)),
                        before,
                        "glyphs moved on focus at {dpi} DPI"
                    );
                    let _ = SetFocus(Some(window));
                    assert_eq!(
                        surface_pixels(&print_control(control)),
                        before,
                        "glyphs moved on blur at {dpi} DPI"
                    );
                    assert_ne!(GetWindowLongW(control, GWL_STYLE) as u32 & WS_VISIBLE.0, 0);
                    let mut rect_after = RECT::default();
                    SendMessageW(
                        control,
                        EM_GETRECT,
                        None,
                        Some(LPARAM(ptr::from_mut(&mut rect_after) as isize)),
                    );
                    assert_eq!(rect_before, rect_after);
                }
                if let Some(directory) = std::env::var_os("NOTY_RENDER_PREVIEW_DIR") {
                    let client = RECT {
                        left: 0,
                        top: 0,
                        right: scale_for_dpi(EDITOR_WIDTH_DIP, dpi),
                        bottom: scale_for_dpi(EDITOR_HEIGHT_DIP, dpi),
                    };
                    let renderer = EditorRenderer::new(client.right, client.bottom).unwrap();
                    renderer.render(client, dpi).unwrap();
                    let mut parent_rect = RECT::default();
                    GetWindowRect(window, &mut parent_rect).unwrap();
                    for control in [title, body] {
                        let surface = print_control(control);
                        let mut control_rect = RECT::default();
                        GetWindowRect(control, &mut control_rect).unwrap();
                        BitBlt(
                            renderer.surface.dc,
                            control_rect.left - parent_rect.left,
                            control_rect.top - parent_rect.top,
                            surface.width,
                            surface.height,
                            Some(surface.dc),
                            0,
                            0,
                            SRCCOPY,
                        )
                        .unwrap();
                    }
                    write_preview_bitmap(
                        &std::path::PathBuf::from(directory).join(format!("editor-{dpi}dpi.bmp")),
                        &renderer.surface,
                    );
                }
                let content = (0..40)
                    .map(|line| format!("第 {line:02} 行：输入不会重排文字。\r\n"))
                    .collect::<String>();
                SetWindowTextW(body, PCWSTR(utf16(&content).as_ptr())).unwrap();
                let _ = SetFocus(Some(body));
                let end = content.encode_utf16().count();
                SendMessageW(
                    body,
                    EM_SETSEL,
                    Some(WPARAM(end)),
                    Some(LPARAM(end as isize)),
                );
                SendMessageW(body, EM_SCROLLCARET, None, None);
                let line = SendMessageW(body, EM_GETFIRSTVISIBLELINE, None, None);
                assert!(line.0 > 0);
                let before = surface_pixels(&print_control(body));
                let _ = SetFocus(Some(title));
                let _ = SetFocus(Some(body));
                assert_eq!(SendMessageW(body, EM_GETFIRSTVISIBLELINE, None, None), line);
                assert_eq!(surface_pixels(&print_control(body)), before);
                let suffix = utf16("继续输入");
                EDITOR_LOADING.store(false, Ordering::Release);
                SendMessageW(
                    body,
                    EM_REPLACESEL,
                    Some(WPARAM(1)),
                    Some(LPARAM(suffix.as_ptr() as isize)),
                );
                assert_eq!(window_text(body), format!("{content}继续输入"));
                assert!(IsWindowVisible(body).as_bool());
                assert!(EDITOR_DIRTY.load(Ordering::Acquire));
                let _ = KillTimer(Some(window), EDITOR_SAVE_TIMER_ID);
                EDITOR_LOADING.store(true, Ordering::Release);
                SetWindowTextW(body, PCWSTR(utf16("").as_ptr())).unwrap();
                let _ = SetFocus(Some(title));
                assert!(IsWindowVisible(load_window(&EDITOR_PLACEHOLDER)).as_bool());
                handle_editor_command(window, WPARAM(EDITOR_PLACEHOLDER_ID));
                assert_eq!(GetFocus(), body);
                assert!(!IsWindowVisible(load_window(&EDITOR_PLACEHOLDER)).as_bool());
                assert!(IsWindowVisible(body).as_bool());
            }
            drop(editor);
        }
    }

    fn write_preview_bitmap(path: &std::path::Path, surface: &OpaqueSurface) {
        let pixels = surface_pixels(surface);
        let mut bytes = Vec::with_capacity(54 + pixels.len());
        bytes.extend_from_slice(b"BM");
        bytes.extend_from_slice(&((54 + pixels.len()) as u32).to_le_bytes());
        bytes.extend_from_slice(&[0; 4]);
        bytes.extend_from_slice(&54_u32.to_le_bytes());
        bytes.extend_from_slice(&40_u32.to_le_bytes());
        bytes.extend_from_slice(&surface.width.to_le_bytes());
        bytes.extend_from_slice(&(-surface.height).to_le_bytes());
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(&32_u16.to_le_bytes());
        bytes.extend_from_slice(&[0; 24]);
        bytes.extend_from_slice(&pixels);
        std::fs::write(path, bytes).unwrap();
    }

    #[test]
    fn changing_card_titles_reuses_antialiased_geometry() {
        let key = CardShapeKey {
            color: "purple".to_string(),
            resting_width: 125,
            card_height: 65,
            dpi: 96,
        };
        let before = card_shape_variants(&key);
        let note = NoteSummary {
            id: "test".to_string(),
            title: "新标题".to_string(),
            content: String::new(),
            color: key.color.clone(),
            is_pinned: false,
        };
        // SAFETY: Text rasterization is isolated to this test thread.
        let sprites = unsafe { build_card_sprites(142, 96, &[note], None).unwrap() };
        assert!(Arc::ptr_eq(&before, &card_shape_variants(&key)));
        assert_eq!(sprites[0].len(), before.len());
        assert_ne!(sprites[0][0].pixels, before[0].pixels);
    }

    #[test]
    fn editor_small_text_has_antialiasing_without_colored_fringes() {
        // SAFETY: The test renders only into its own memory DC.
        unsafe {
            for dpi in [96, 120, 144, 192] {
                let width = scale_for_dpi(160, dpi);
                let height = scale_for_dpi(30, dpi);
                let renderer = EditorRenderer::new(width, height).unwrap();
                let rect = RECT {
                    left: 0,
                    top: 0,
                    right: width,
                    bottom: height,
                };
                renderer.target.BindDC(renderer.surface.dc, &rect).unwrap();
                renderer.target.BeginDraw();
                renderer
                    .target
                    .Clear(Some(&d2d_color(Color::new(255, 255, 255))));
                draw_editor_text(
                    &renderer,
                    "速记胶囊 已保存 35 字符",
                    rect,
                    TextStyle {
                        face: UI_FONT_FACE,
                        pixel_height: scale_for_dpi(EDITOR_CHROME_FONT_DIP, dpi),
                        weight: NORMAL_FONT_WEIGHT,
                        color: Color::new(0, 0, 0),
                    },
                    DWRITE_TEXT_ALIGNMENT_LEADING,
                    DWRITE_PARAGRAPH_ALIGNMENT_CENTER,
                    DWRITE_WORD_WRAPPING_NO_WRAP,
                    true,
                )
                .unwrap();
                renderer.target.EndDraw(None, None).unwrap();
                let pixels = surface_pixels(&renderer.surface);
                assert!(pixels.chunks_exact(4).any(|pixel| pixel[0] < 100));
                assert!(
                    pixels
                        .chunks_exact(4)
                        .any(|pixel| pixel[0] > 0 && pixel[0] < 255)
                );
                for pixel in pixels.chunks_exact(4) {
                    assert_eq!(pixel[0], pixel[1], "blue/green fringe at {dpi} DPI");
                    assert_eq!(pixel[1], pixel[2], "green/red fringe at {dpi} DPI");
                }
            }
        }
    }

    #[test]
    fn editor_icon_strokes_align_to_physical_pixel_edges() {
        for dpi in [96, 120, 144, 168, 192, 240, 288] {
            let stroke = scale_for_dpi(1, dpi).max(1) as f32;
            for center in [30.0, 291.0, 328.0] {
                let position = snap_stroke_center(center * dpi as f32 / 96.0, stroke);
                assert_eq!((position - stroke / 2.0).fract(), 0.0);
                assert_eq!((position + stroke / 2.0).fract(), 0.0);
            }
        }
    }

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
    fn list_scroll_range_starts_after_six_notes() {
        assert_eq!(max_scroll_offset(6), 0);
        assert_eq!(max_scroll_offset(7), 1);
        assert_eq!(max_scroll_offset(12), 6);
        assert_eq!(max_scroll_position(6, 96), 0.0);
        assert_eq!(max_scroll_position(7, 96), 78.0);
        assert_eq!(max_scroll_position(12, 96), 318.0);
    }

    #[test]
    fn list_viewport_matches_reference_geometry() {
        assert_eq!(list_height(2, 96), 210);
        assert_eq!(list_height(3, 96), 258);
        assert_eq!(list_height(4, 96), 306);
        assert_eq!(list_height(5, 96), 354);
        assert_eq!(list_height(20, 96), 354);
        assert_eq!(card_viewport_bottom(20, 96), 296);
        assert_eq!(footer_y(20, 96), 306);
    }
}
