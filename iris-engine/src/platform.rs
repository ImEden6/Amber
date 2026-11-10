use crate::gamma::GammaRamp;

#[cfg(target_os = "windows")]
mod imp {
    use super::*;
    use std::ffi::c_void;

    type HWND = *mut c_void;
    type HDC = *mut c_void;
    type BOOL = i32;

    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetDC(hwnd: HWND) -> HDC;
        fn ReleaseDC(hwnd: HWND, hdc: HDC) -> i32;
    }

    #[link(name = "gdi32")]
    unsafe extern "system" {
        fn GetDeviceGammaRamp(hdc: HDC, lp_ramp: *mut c_void) -> BOOL;
        fn SetDeviceGammaRamp(hdc: HDC, lp_ramp: *const c_void) -> BOOL;
    }

    pub struct DeviceContext(pub HDC);

    impl DeviceContext {
        pub fn desktop() -> Result<Self, String> {
            let hdc = unsafe { GetDC(std::ptr::null_mut()) };
            if hdc.is_null() {
                Err("Failed to acquire Windows desktop DC via GetDC".to_string())
            } else {
                Ok(Self(hdc))
            }
        }
    }

    impl Drop for DeviceContext {
        fn drop(&mut self) {
            unsafe {
                ReleaseDC(std::ptr::null_mut(), self.0);
            }
        }
    }

    pub struct GammaSession(DeviceContext);

    impl GammaSession {
        pub fn open() -> Result<Self, String> {
            DeviceContext::desktop().map(Self)
        }

        pub fn set_ramp(&self, ramp: &GammaRamp) -> Result<(), String> {
            let success = unsafe {
                SetDeviceGammaRamp(self.0 .0, ramp as *const GammaRamp as *const c_void)
            };
            if success != 0 {
                Ok(())
            } else {
                Err("Win32 SetDeviceGammaRamp returned FALSE".to_string())
            }
        }
    }

    pub fn get_gamma_ramp() -> Result<GammaRamp, String> {
        let dc = DeviceContext::desktop()?;
        let mut ramp = GammaRamp::default();
        let success = unsafe {
            GetDeviceGammaRamp(dc.0, &mut ramp as *mut GammaRamp as *mut c_void)
        };
        if success != 0 {
            Ok(ramp)
        } else {
            Err("Win32 GetDeviceGammaRamp returned FALSE".to_string())
        }
    }

    pub fn set_gamma_ramp(ramp: &GammaRamp) -> Result<(), String> {
        let dc = DeviceContext::desktop()?;
        let success = unsafe {
            SetDeviceGammaRamp(dc.0, ramp as *const GammaRamp as *const c_void)
        };
        if success != 0 {
            Ok(())
        } else {
            Err("Win32 SetDeviceGammaRamp returned FALSE".to_string())
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    use super::*;
    use std::sync::Mutex;

    static MOCK_RAMP: Mutex<Option<GammaRamp>> = Mutex::new(None);

    pub struct GammaSession;
    impl GammaSession {
        pub fn open() -> Result<Self, String> {
            Ok(Self)
        }
        pub fn set_ramp(&self, ramp: &GammaRamp) -> Result<(), String> {
            set_gamma_ramp(ramp)
        }
    }

    pub fn get_gamma_ramp() -> Result<GammaRamp, String> {
        let guard = MOCK_RAMP.lock().unwrap();
        Ok(guard.unwrap_or_else(GammaRamp::linear))
    }

    pub fn set_gamma_ramp(ramp: &GammaRamp) -> Result<(), String> {
        let mut guard = MOCK_RAMP.lock().unwrap();
        *guard = Some(*ramp);
        Ok(())
    }
}

pub use imp::{get_gamma_ramp, set_gamma_ramp, GammaSession};
