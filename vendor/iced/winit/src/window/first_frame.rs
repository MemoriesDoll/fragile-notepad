//! Supply a prepared frame to the native Windows opening animation.
//!
//! Render offscreen before showing the HWND, then paint those pixels from its
//! native erase/print callbacks until normal presentation takes over. Hidden
//! HWNDs do not receive the ordinary WM_PAINT events that drive Iced redraws.

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

    pub fn needs_prepare(&self) -> bool {
        #[cfg(target_os = "windows")]
        {
            self.installed
        }
        #[cfg(not(target_os = "windows"))]
        {
            false
        }
    }

    pub fn prepare(
        &mut self,
        window: &winit::window::Window,
        size: crate::core::Size<u32>,
        pixels: Vec<u8>,
    ) {
        #[cfg(target_os = "windows")]
        if self.installed {
            platform::prepare(window, size, pixels);
        }
        #[cfg(not(target_os = "windows"))]
        let _ = (window, size, pixels);
    }
}

#[cfg(target_os = "windows")]
#[allow(unsafe_code)]
mod platform {
    use windows_sys::Win32::{
        Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
        Graphics::Gdi::{
            BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateSolidBrush, DIB_RGB_COLORS, DeleteObject,
            FillRect, GDI_ERROR, GdiFlush, HDC, SRCCOPY, StretchDIBits,
        },
        UI::{
            Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
            WindowsAndMessaging::{
                GetClientRect, GetPropW, RemovePropW, SetPropW, WM_ERASEBKGND, WM_NCDESTROY,
                WM_PRINTCLIENT,
            },
        },
    };
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

    const BACKGROUND_PROPERTY: windows_sys::core::PCWSTR =
        windows_sys::w!("Iced.FirstFrame.Background");

    struct Background {
        color: u32,
        frame: Option<Frame>,
    }

    struct Frame {
        width: i32,
        height: i32,
        bgra: Vec<u8>,
    }

    impl Frame {
        fn from_rgba(size: crate::core::Size<u32>, mut pixels: Vec<u8>) -> Option<Self> {
            let width = i32::try_from(size.width).ok().filter(|width| *width > 0)?;
            let height = i32::try_from(size.height)
                .ok()
                .filter(|height| *height > 0)?;
            let bytes = (width as usize)
                .checked_mul(height as usize)?
                .checked_mul(4)?;
            if pixels.len() != bytes {
                return None;
            }
            for pixel in pixels.chunks_exact_mut(4) {
                pixel.swap(0, 2);
            }
            Some(Self {
                width,
                height,
                bgra: pixels,
            })
        }

        unsafe fn paint(&self, dc: HDC, rect: &RECT) -> bool {
            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: self.width,
                    biHeight: -self.height,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB,
                    ..Default::default()
                },
                ..Default::default()
            };
            // SAFETY: from_rgba validated the buffer length and dimensions.
            // A negative DIB height matches the renderer's top-to-bottom rows.
            let copied = unsafe {
                StretchDIBits(
                    dc,
                    rect.left,
                    rect.top,
                    rect.right - rect.left,
                    rect.bottom - rect.top,
                    0,
                    0,
                    self.width,
                    self.height,
                    self.bgra.as_ptr().cast(),
                    &info,
                    DIB_RGB_COLORS,
                    SRCCOPY,
                )
            };
            copied != 0 && copied != GDI_ERROR
        }
    }

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
        install_on_hwnd(hwnd, color)
    }

    fn install_on_hwnd(hwnd: HWND, color: u32) -> bool {
        let background = Box::into_raw(Box::new(Background { color, frame: None }));
        // SAFETY: installation, updates, and removal run on the HWND's thread.
        // The subclass owns this allocation until removal or WM_NCDESTROY.
        let installed = unsafe {
            if SetPropW(hwnd, BACKGROUND_PROPERTY, background.cast()) == 0 {
                false
            } else if SetWindowSubclass(hwnd, Some(background_proc), 0, background as usize) == 0 {
                let _ = RemovePropW(hwnd, BACKGROUND_PROPERTY);
                false
            } else {
                true
            }
        };
        if !installed {
            // SAFETY: a failed installation did not transfer ownership.
            drop(unsafe { Box::from_raw(background) });
            log::warn!("Could not install initial window background handler");
        }
        installed
    }

    pub fn remove(window: &winit::window::Window) -> bool {
        let Some(hwnd) = hwnd(window) else {
            return false;
        };
        remove_from_hwnd(hwnd)
    }

    fn remove_from_hwnd(hwnd: HWND) -> bool {
        // SAFETY: the callback/ID identify our allocation. Detach before freeing,
        // so subsequent native messages cannot reference the old frame.
        unsafe {
            let data = GetPropW(hwnd, BACKGROUND_PROPERTY);
            if data.is_null() {
                return true;
            }
            if RemoveWindowSubclass(hwnd, Some(background_proc), 0) == 0 {
                return false;
            }
            let _ = RemovePropW(hwnd, BACKGROUND_PROPERTY);
            drop(Box::from_raw(data as *mut Background));
        }
        true
    }

    pub fn prepare(window: &winit::window::Window, size: crate::core::Size<u32>, pixels: Vec<u8>) {
        let Some(hwnd) = hwnd(window) else { return };
        prepare_on_hwnd(hwnd, size, pixels);
    }

    fn prepare_on_hwnd(hwnd: HWND, size: crate::core::Size<u32>, pixels: Vec<u8>) {
        let Some(frame) = Frame::from_rgba(size, pixels) else {
            return;
        };
        // SAFETY: the hidden HWND's subclass owns Background. This same-thread
        // update makes no native calls while mutating it, so cannot reenter paint.
        unsafe {
            let data = GetPropW(hwnd, BACKGROUND_PROPERTY);
            if !data.is_null() {
                (*(data as *mut Background)).frame = Some(frame);
            }
        }
    }

    unsafe extern "system" fn background_proc(
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        _id: usize,
        data: usize,
    ) -> LRESULT {
        // SAFETY: Windows supplies a live HWND and, for WM_ERASEBKGND, its HDC.
        // The subclass owns data until detachment. Native visibility and
        // animation messages retain winit's normal handling.
        unsafe {
            if message == WM_ERASEBKGND || message == WM_PRINTCLIENT {
                let mut rect = RECT::default();
                if GetClientRect(hwnd, &mut rect) == 0 {
                    return DefSubclassProc(hwnd, message, wparam, lparam);
                }
                let background = &*(data as *const Background);
                if let Some(frame) = &background.frame
                    && frame.paint(wparam as _, &rect)
                {
                    let _ = GdiFlush();
                    return 1;
                }
                let brush = CreateSolidBrush(background.color);
                if !brush.is_null() {
                    let filled = FillRect(wparam as _, &rect, brush);
                    let _ = DeleteObject(brush);
                    let _ = GdiFlush();
                    if filled != 0 {
                        return 1;
                    }
                }
            } else if message == WM_NCDESTROY {
                let _ = remove_from_hwnd(hwnd);
            }
            DefSubclassProc(hwnd, message, wparam, lparam)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use windows_sys::Win32::{
            Graphics::Gdi::{
                CreateCompatibleDC, CreateDIBSection, DeleteDC, HBITMAP, HGDIOBJ, SelectObject,
            },
            UI::WindowsAndMessaging::{
                CreateWindowExW, DestroyWindow, IsWindowVisible, SendMessageW, WS_POPUP,
            },
        };

        // Real native messages against a hidden HWND and an in-memory DC. No
        // desktop window is shown and no capture files are produced.
        struct Target {
            hwnd: HWND,
            dc: HDC,
            bitmap: HBITMAP,
            previous: HGDIOBJ,
            pixels: *mut u8,
        }

        impl Target {
            fn new() -> Self {
                unsafe {
                    let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
                    let hwnd = CreateWindowExW(
                        0,
                        class.as_ptr(),
                        std::ptr::null(),
                        WS_POPUP,
                        0,
                        0,
                        2,
                        2,
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        std::ptr::null(),
                    );
                    assert!(!hwnd.is_null());
                    let dc = CreateCompatibleDC(std::ptr::null_mut());
                    assert!(!dc.is_null());
                    let info = BITMAPINFO {
                        bmiHeader: BITMAPINFOHEADER {
                            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as _,
                            biWidth: 2,
                            biHeight: -2,
                            biPlanes: 1,
                            biBitCount: 32,
                            biCompression: BI_RGB,
                            ..Default::default()
                        },
                        ..Default::default()
                    };
                    let mut pixels = std::ptr::null_mut();
                    let bitmap = CreateDIBSection(
                        dc,
                        &info,
                        DIB_RGB_COLORS,
                        &mut pixels,
                        std::ptr::null_mut(),
                        0,
                    );
                    assert!(!bitmap.is_null());
                    let previous = SelectObject(dc, bitmap);
                    Self {
                        hwnd,
                        dc,
                        bitmap,
                        previous,
                        pixels: pixels.cast(),
                    }
                }
            }

            fn paint(&self, message: u32) -> Vec<[u8; 3]> {
                unsafe {
                    assert_eq!(IsWindowVisible(self.hwnd), 0);
                    assert_eq!(SendMessageW(self.hwnd, message, self.dc as usize, 0), 1);
                    std::slice::from_raw_parts(self.pixels, 16)
                        .chunks_exact(4)
                        .map(|pixel| [pixel[2], pixel[1], pixel[0]])
                        .collect()
                }
            }
        }

        impl Drop for Target {
            fn drop(&mut self) {
                unsafe {
                    let _ = SelectObject(self.dc, self.previous);
                    let _ = DeleteObject(self.bitmap);
                    let _ = DeleteDC(self.dc);
                    if !self.hwnd.is_null() {
                        let _ = DestroyWindow(self.hwnd);
                    }
                }
            }
        }

        #[test]
        fn hidden_window_paints_prepared_pixels_in_native_channel_and_row_order() {
            let target = Target::new();
            assert!(install_on_hwnd(target.hwnd, 0x00332211));
            assert_eq!(target.paint(WM_ERASEBKGND), vec![[0x11, 0x22, 0x33]; 4]);

            prepare_on_hwnd(
                target.hwnd,
                crate::core::Size::new(2, 2),
                vec![
                    255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 24, 48, 96, 255,
                ],
            );
            let expected = vec![[255, 0, 0], [0, 255, 0], [0, 0, 255], [24, 48, 96]];
            assert_eq!(target.paint(WM_ERASEBKGND), expected);
            assert_eq!(target.paint(WM_PRINTCLIENT), expected);
            assert!(remove_from_hwnd(target.hwnd));
            assert!(unsafe { GetPropW(target.hwnd, BACKGROUND_PROPERTY) }.is_null());
            assert!(remove_from_hwnd(target.hwnd));
        }

        #[test]
        fn invalid_frame_keeps_background_and_destroy_detaches_handler() {
            let mut target = Target::new();
            assert!(install_on_hwnd(target.hwnd, 0x00665544));
            prepare_on_hwnd(target.hwnd, crate::core::Size::new(2, 2), vec![0; 3]);
            assert_eq!(target.paint(WM_ERASEBKGND), vec![[0x44, 0x55, 0x66]; 4]);
            // Exercise the destroy-before-presentation cleanup path.
            assert_ne!(unsafe { DestroyWindow(target.hwnd) }, 0);
            target.hwnd = std::ptr::null_mut();
        }
    }
}
