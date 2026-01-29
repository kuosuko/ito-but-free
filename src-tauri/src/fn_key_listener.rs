use std::sync::{Arc, Mutex};

#[cfg(target_os = "macos")]
pub mod macos {
    use super::*;
    use std::ffi::c_void;

    #[repr(C)]
    struct CFRunLoopSource;
    #[repr(C)]
    struct CFMachPort;
    #[repr(C)]
    struct CFRunLoop;
    #[repr(C)]
    struct CGEvent;

    type CGEventTapCallBack = extern "C" fn(
        proxy: *mut c_void,
        event_type: u32,
        event: *mut CGEvent,
        user_info: *mut c_void,
    ) -> *mut CGEvent;

    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGEventTapCreate(
            tap: u32,
            place: u32,
            options: u32,
            events_of_interest: u64,
            callback: CGEventTapCallBack,
            user_info: *mut c_void,
        ) -> *mut CFMachPort;

        fn CGEventTapEnable(tap: *mut CFMachPort, enable: bool);

        fn CFMachPortCreateRunLoopSource(
            allocator: *const c_void,
            tap: *mut CFMachPort,
            order: isize,
        ) -> *mut CFRunLoopSource;

        fn CFRunLoopGetCurrent() -> *mut CFRunLoop;
        fn CFRunLoopAddSource(rl: *mut CFRunLoop, source: *mut CFRunLoopSource, mode: *const c_void);

        fn CGEventGetFlags(event: *const CGEvent) -> u64;
        fn CGEventGetType(event: *const CGEvent) -> u32;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        static kCFRunLoopCommonModes: *const c_void;
    }

    // CGEventMask for flagsChanged
    const K_CG_EVENT_FLAGS_CHANGED: u32 = 12;
    const K_CG_EVENT_MASK_FLAGS_CHANGED: u64 = 1 << K_CG_EVENT_FLAGS_CHANGED;

    // CGEventTapLocation
    const K_CG_HID_EVENT_TAP: u32 = 0;
    // CGEventTapPlacement
    const K_CG_HEAD_INSERT_EVENT_TAP: u32 = 0;
    // CGEventTapOptions
    const K_CG_EVENT_TAP_OPTION_DEFAULT: u32 = 0;

    // CGEventFlags - Fn key flag
    const K_CG_EVENT_FLAG_MASK_SECONDARY_FN: u64 = 0x800000;

    pub struct FnKeyListener {
        tap: *mut CFMachPort,
        callback: Arc<Mutex<Box<dyn FnMut(bool) + Send>>>,
    }

    unsafe impl Send for FnKeyListener {}
    unsafe impl Sync for FnKeyListener {}

    extern "C" fn event_tap_callback(
        _proxy: *mut c_void,
        event_type: u32,
        event: *mut CGEvent,
        user_info: *mut c_void,
    ) -> *mut CGEvent {
        unsafe {
            if event_type == K_CG_EVENT_FLAGS_CHANGED {
                let flags = CGEventGetFlags(event);
                let fn_pressed = (flags & K_CG_EVENT_FLAG_MASK_SECONDARY_FN) != 0;

                if !user_info.is_null() {
                    let callback_ptr = user_info as *mut Arc<Mutex<Box<dyn FnMut(bool) + Send>>>;
                    if let Some(callback_arc) = callback_ptr.as_ref() {
                        if let Ok(mut cb) = callback_arc.lock() {
                            cb(fn_pressed);
                        }
                    }
                }
            }
            event
        }
    }

    impl FnKeyListener {
        pub fn new<F>(callback: F) -> Result<Self, String>
        where
            F: FnMut(bool) + Send + 'static,
        {
            unsafe {
                let callback_arc = Arc::new(Mutex::new(Box::new(callback) as Box<dyn FnMut(bool) + Send>));
                let user_info = Box::into_raw(Box::new(callback_arc.clone())) as *mut c_void;

                let tap = CGEventTapCreate(
                    K_CG_HID_EVENT_TAP,
                    K_CG_HEAD_INSERT_EVENT_TAP,
                    K_CG_EVENT_TAP_OPTION_DEFAULT,
                    K_CG_EVENT_MASK_FLAGS_CHANGED,
                    event_tap_callback,
                    user_info,
                );

                if tap.is_null() {
                    // Clean up user_info if tap creation failed
                    let _ = Box::from_raw(user_info);
                    return Err("Failed to create event tap. Please:\n1. Completely quit this app (Cmd+Q)\n2. Reopen it\n3. Try enabling Fn key again\n\nNote: macOS requires app restart after granting Accessibility permission.".into());
                }

                let run_loop_source = CFMachPortCreateRunLoopSource(std::ptr::null(), tap, 0);
                if run_loop_source.is_null() {
                    return Err("Failed to create run loop source".into());
                }

                let run_loop = CFRunLoopGetCurrent();
                CFRunLoopAddSource(run_loop, run_loop_source, kCFRunLoopCommonModes);

                CGEventTapEnable(tap, true);

                Ok(FnKeyListener {
                    tap,
                    callback: callback_arc,
                })
            }
        }

        pub fn stop(&self) {
            unsafe {
                if !self.tap.is_null() {
                    CGEventTapEnable(self.tap, false);
                }
            }
        }
    }

    impl Drop for FnKeyListener {
        fn drop(&mut self) {
            self.stop();
        }
    }
}

#[cfg(target_os = "macos")]
pub use macos::FnKeyListener;

#[cfg(not(target_os = "macos"))]
pub struct FnKeyListener;

#[cfg(not(target_os = "macos"))]
impl FnKeyListener {
    pub fn new<F>(_callback: F) -> Result<Self, String>
    where
        F: FnMut(bool) + Send + 'static,
    {
        Err("Fn key listening is only supported on macOS".into())
    }

    pub fn stop(&self) {}
}
