//! Click-through transparent hardware/software overlay for true sub-monitor dimming.
//! Bypasses all GPU driver gamma clamping and works across every monitor, OLED, and HDR.

#[cfg(target_os = "windows")]
mod imp {
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc::channel;
    use std::sync::Arc;
    use std::thread::{self, JoinHandle};

    type HWND = *mut c_void;
    type HINSTANCE = *mut c_void;
    type HBRUSH = *mut c_void;
    type HCURSOR = *mut c_void;
    type HICON = *mut c_void;
    type ATOM = u16;
    type LRESULT = isize;
    type UINT = u32;
    type WPARAM = usize;
    type LPARAM = isize;
    type BOOL = i32;
    type COLORREF = u32;

    const WS_EX_LAYERED: u32 = 0x00080000;
    const WS_EX_TRANSPARENT: u32 = 0x00000020;
    const WS_EX_TOPMOST: u32 = 0x00000008;
    const WS_EX_TOOLWINDOW: u32 = 0x00000080;
    const WS_EX_NOACTIVATE: u32 = 0x08000000;
    const WS_POPUP: u32 = 0x80000000;
    const SW_SHOW: i32 = 5;
    const SW_HIDE: i32 = 0;
    const LWA_ALPHA: u32 = 0x00000002;
    const WM_USER: u32 = 0x0400;
    const WM_SET_ALPHA: u32 = WM_USER + 1;
    const WM_DESTROY: u32 = 0x0002;

    #[repr(C)]
    struct WNDCLASSEXW {
        cb_size: u32,
        style: u32,
        lpfn_wnd_proc: unsafe extern "system" fn(HWND, UINT, WPARAM, LPARAM) -> LRESULT,
        cb_cls_extra: i32,
        cb_wnd_extra: i32,
        h_instance: HINSTANCE,
        h_icon: HICON,
        h_cursor: HCURSOR,
        hbr_background: HBRUSH,
        lpsz_menu_name: *const u16,
        lpsz_class_name: *const u16,
        h_icon_sm: HICON,
    }

    #[repr(C)]
    struct POINT {
        x: i32,
        y: i32,
    }

    #[repr(C)]
    struct MSG {
        hwnd: HWND,
        message: UINT,
        w_param: WPARAM,
        l_param: LPARAM,
        time: u32,
        pt: POINT,
    }

    #[link(name = "user32")]
    unsafe extern "system" {
        fn RegisterClassExW(lp_class: *const WNDCLASSEXW) -> ATOM;
        fn CreateWindowExW(
            dw_ex_style: u32,
            lp_class_name: *const u16,
            lp_window_name: *const u16,
            dw_style: u32,
            x: i32,
            y: i32,
            n_width: i32,
            n_height: i32,
            h_parent: HWND,
            h_menu: *mut c_void,
            h_instance: HINSTANCE,
            lp_param: *mut c_void,
        ) -> HWND;
        fn DefWindowProcW(hwnd: HWND, msg: UINT, w_param: WPARAM, l_param: LPARAM) -> LRESULT;
        fn SetLayeredWindowAttributes(hwnd: HWND, cr_key: COLORREF, b_alpha: u8, dw_flags: u32) -> BOOL;
        fn ShowWindow(hwnd: HWND, n_cmd_show: i32) -> BOOL;
        fn DestroyWindow(hwnd: HWND) -> BOOL;
        fn PostMessageW(hwnd: HWND, msg: UINT, w_param: WPARAM, l_param: LPARAM) -> BOOL;
        fn PostQuitMessage(n_exit_code: i32);
        fn GetMessageW(lp_msg: *mut MSG, hwnd: HWND, w_msg_filter_min: UINT, w_msg_filter_max: UINT) -> BOOL;
        fn TranslateMessage(lp_msg: *const MSG) -> BOOL;
        fn DispatchMessageW(lp_msg: *const MSG) -> LRESULT;
        fn GetSystemMetrics(n_index: i32) -> i32;
        fn SetWindowPos(hwnd: HWND, after: HWND, x: i32, y: i32, cx: i32, cy: i32, flags: UINT) -> BOOL;
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetModuleHandleW(name: *const u16) -> HINSTANCE;
        fn GetProcAddress(module: HINSTANCE, name: *const u8) -> *mut c_void;
    }

    const DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2: isize = -4;
    const WM_ENDSESSION: u32 = 0x0016;
    const WM_DISPLAYCHANGE: u32 = 0x007E;
    const WM_DPICHANGED: u32 = 0x02E0;
    const HWND_TOPMOST: isize = -1;
    const SWP_NOACTIVATE: u32 = 0x0010;

    fn use_per_monitor_dpi() {
        let user32: Vec<u16> = "user32.dll\0".encode_utf16().collect();
        unsafe {
            let proc = GetProcAddress(GetModuleHandleW(user32.as_ptr()), c"SetThreadDpiAwarenessContext".as_ptr() as *const u8);
            if !proc.is_null() {
                let set_ctx: unsafe extern "system" fn(*mut c_void) -> *mut c_void = std::mem::transmute(proc);
                set_ctx(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2 as *mut c_void);
            }
        }
    }

    fn virtual_screen() -> (i32, i32, i32, i32) {
        unsafe { (GetSystemMetrics(76), GetSystemMetrics(77), GetSystemMetrics(78), GetSystemMetrics(79)) }
    }

    #[link(name = "gdi32")]
    unsafe extern "system" {
        fn CreateSolidBrush(color: COLORREF) -> HBRUSH;
        fn DeleteObject(ho: *mut c_void) -> BOOL;
    }

    unsafe extern "system" fn window_proc(hwnd: HWND, msg: UINT, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
        match msg {
            WM_SET_ALPHA => {
                let alpha = (w_param & 0xFF) as u8;
                unsafe {
                    if alpha == 0 {
                        ShowWindow(hwnd, SW_HIDE);
                    } else {
                        SetLayeredWindowAttributes(hwnd, 0, alpha, LWA_ALPHA);
                        ShowWindow(hwnd, SW_SHOW);
                    }
                }
                0
            }
            WM_DISPLAYCHANGE | WM_DPICHANGED => {
                let (x, y, w, h) = virtual_screen();
                unsafe {
                    SetWindowPos(hwnd, HWND_TOPMOST as HWND, x, y, w, h, SWP_NOACTIVATE);
                }
                0
            }
            WM_ENDSESSION => {
                if w_param != 0 {
                    crate::engine::restore_on_shutdown();
                }
                0
            }
            WM_DESTROY => {
                unsafe {
                    PostQuitMessage(0);
                }
                0
            }
            _ => unsafe { DefWindowProcW(hwnd, msg, w_param, l_param) },
        }
    }

    /// Click-through overlay controller running on a dedicated UI message pump thread.
    pub struct ScreenDimmer {
        hwnd: usize,
        is_running: Arc<AtomicBool>,
        #[allow(dead_code)]
        thread_handle: Option<JoinHandle<()>>,
    }

    impl ScreenDimmer {
        pub fn new() -> Self {
            let is_running = Arc::new(AtomicBool::new(true));
            let (tx, rx) = channel::<usize>();

            let is_running_clone = Arc::clone(&is_running);

            let handle = thread::spawn(move || {
                use_per_monitor_dpi();

                let class_name: Vec<u16> = "AmberDimmerOverlay\0".encode_utf16().collect();
                let black_brush = unsafe { CreateSolidBrush(0x00000000) };

                let wc = WNDCLASSEXW {
                    cb_size: std::mem::size_of::<WNDCLASSEXW>() as u32,
                    style: 0,
                    lpfn_wnd_proc: window_proc,
                    cb_cls_extra: 0,
                    cb_wnd_extra: 0,
                    h_instance: std::ptr::null_mut(),
                    h_icon: std::ptr::null_mut(),
                    h_cursor: std::ptr::null_mut(),
                    hbr_background: black_brush,
                    lpsz_menu_name: std::ptr::null(),
                    lpsz_class_name: class_name.as_ptr(),
                    h_icon_sm: std::ptr::null_mut(),
                };

                unsafe {
                    RegisterClassExW(&wc);
                }

                let (x, y, w, h) = virtual_screen();

                let ex_style = WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE;
                let hwnd = unsafe {
                    CreateWindowExW(
                        ex_style,
                        class_name.as_ptr(),
                        std::ptr::null(),
                        WS_POPUP,
                        x, y, w, h,
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                    )
                };

                unsafe {
                    SetLayeredWindowAttributes(hwnd, 0, 0, LWA_ALPHA);
                }

                let _ = tx.send(hwnd as usize);

                let mut msg: MSG = unsafe { std::mem::zeroed() };
                while is_running_clone.load(Ordering::Relaxed) {
                    unsafe {
                        if GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                            TranslateMessage(&msg);
                            DispatchMessageW(&msg);
                        } else {
                            break;
                        }
                    }
                }

                unsafe {
                    DestroyWindow(hwnd);
                    DeleteObject(black_brush);
                }
            });

            let hwnd = rx.recv().unwrap_or(0);

            Self {
                hwnd,
                is_running,
                thread_handle: Some(handle),
            }
        }

        /// Sets the dimming level (0.0 = 100% normal brightness, 0.9 = 10% brightness / 90% dimmed).
        pub fn set_dimming(&self, dim_ratio: f64) {
            if self.hwnd == 0 {
                return;
            }
            let dim_ratio = dim_ratio.clamp(0.0, 0.90);
            let alpha = (dim_ratio * 255.0).round() as u8;
            unsafe {
                PostMessageW(self.hwnd as HWND, WM_SET_ALPHA, alpha as usize, 0);
            }
        }

        /// Clears all dimming overlay.
        pub fn clear(&self) {
            self.set_dimming(0.0);
        }
    }

    impl Drop for ScreenDimmer {
        fn drop(&mut self) {
            self.clear();
            self.is_running.store(false, Ordering::Relaxed);
            if self.hwnd != 0 {
                unsafe {
                    PostMessageW(self.hwnd as HWND, WM_DESTROY, 0, 0);
                }
            }
            if let Some(handle) = self.thread_handle.take() {
                let _ = handle.join();
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    pub struct ScreenDimmer;
    impl ScreenDimmer {
        pub fn new() -> Self { Self }
        pub fn set_dimming(&self, _ratio: f64) {}
        pub fn clear(&self) {}
    }
}

pub use imp::ScreenDimmer;
