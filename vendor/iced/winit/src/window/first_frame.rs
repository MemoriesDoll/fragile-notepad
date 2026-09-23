//! Give the native opening animation a theme-colored background until first paint.
//!
//! Winit uses a null class brush. DefWindowProc can leave the initial client
//! surface white while Iced prepares its first frame. Handle background erasure
//! per HWND until a frame is presented; keep native visibility and transitions.

#[derive(Default)]
pub struct FirstFrame {
    #[cfg(target_os = "windows")]
    installed: bool,
}

impl FirstFrame {
    pub fn new(window: &winit::window::Window, background: crate::core::Color) -> Self {
        // GDI background erasure cannot preserve per-pixel transparency.
        if background.a < 1.0 {
            return Self::default();
        }
        #[cfg(target_os = "windows")]
        {
            Self {
                installed: platform::install(window, background),
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (window, background);
            Self::default()
        }
    }

    pub fn presented(&mut self, window: &winit::window::Window) {
        #[cfg(target_os = "windows")]
        if self.installed && platform::remove(window) {
            self.installed = false;
        }
        #[cfg(not(target_os = "windows"))]
        let _ = window;
    }
}

#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
mod platform {
    use windows_sys::Win32::{
        Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
        Graphics::Gdi::{CreateSolidBrush, DeleteObject, FillRect, GdiFlush},
        UI::{
            Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
            WindowsAndMessaging::{GetClientRect, WM_ERASEBKGND, WM_NCDESTROY},
        },
    };
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

    fn hwnd(window: &winit::window::Window) -> Option<HWND> {
        let RawWindowHandle::Win32(handle) = window.window_handle().ok()?.as_raw() else {
            return None;
        };
        Some(handle.hwnd.get() as _)
    }

    pub fn install(window: &winit::window::Window, background: crate::core::Color) -> bool {
        let Some(hwnd) = hwnd(window) else {
            return false;
        };
        let [r, g, b, _] = background.into_rgba8();
        let color = u32::from(r) | (u32::from(g) << 8) | (u32::from(b) << 16);
        // SAFETY: called on the HWND's event-loop thread. Reference data is a
        // COLORREF value, not a pointer; the callback owns no borrowed state.
        let installed =
            unsafe { SetWindowSubclass(hwnd, Some(background_proc), 0, color as usize) != 0 };
        if !installed {
            log::warn!("Could not install initial window background handler");
        }
        installed
    }

    pub fn remove(window: &winit::window::Window) -> bool {
        let Some(hwnd) = hwnd(window) else {
            return false;
        };
        // SAFETY: the live window and callback/ID match install, on the same thread.
        unsafe { RemoveWindowSubclass(hwnd, Some(background_proc), 0) != 0 }
    }

    unsafe extern "system" fn background_proc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        id: usize,
        color: usize,
    ) -> LRESULT {
        // SAFETY: Windows supplies a live HWND and, for WM_ERASEBKGND, its HDC.
        // Each brush is deleted after the synchronous fill. All other messages
        // retain winit's normal handling, including native fade-in animation.
        unsafe {
            if message == WM_ERASEBKGND {
                let mut rect = RECT::default();
                if GetClientRect(hwnd, &mut rect) == 0 {
                    return DefSubclassProc(hwnd, message, wparam, lparam);
                }
                let brush = CreateSolidBrush(color as u32);
                if !brush.is_null() {
                    let filled = FillRect(wparam as _, &rect, brush);
                    let _ = DeleteObject(brush);
                    let _ = GdiFlush();
                    if filled != 0 {
                        return 1;
                    }
                }
            } else if message == WM_NCDESTROY {
                let _ = RemoveWindowSubclass(hwnd, Some(background_proc), id);
            }
            DefSubclassProc(hwnd, message, wparam, lparam)
        }
    }
}
