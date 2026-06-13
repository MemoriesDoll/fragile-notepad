use crate::ipc::ActivationRequest;

pub(super) fn prepare_for_presentation(_request: &ActivationRequest) {
    activate_macos_application();
}

#[cfg(target_os = "macos")]
fn activate_macos_application() {
    use std::ffi::c_void;
    use std::os::raw::c_char;

    unsafe extern "C" {
        fn objc_getClass(name: *const c_char) -> *mut c_void;
        fn sel_registerName(name: *const c_char) -> *mut c_void;

        #[link_name = "objc_msgSend"]
        fn objc_msgSend_id(receiver: *mut c_void, selector: *mut c_void) -> *mut c_void;

        #[link_name = "objc_msgSend"]
        fn objc_msgSend_void(receiver: *mut c_void, selector: *mut c_void);
    }

    let class_name = c"NSApplication";
    let shared_application = c"sharedApplication";
    let activate_sel = c"activate";

    unsafe {
        let ns_application = objc_getClass(class_name.as_ptr());
        if ns_application.is_null() {
            return;
        }

        let app = objc_msgSend_id(
            ns_application,
            sel_registerName(shared_application.as_ptr()),
        );
        if app.is_null() {
            return;
        }

        objc_msgSend_void(app, sel_registerName(activate_sel.as_ptr()));
    }
}

#[cfg(not(target_os = "macos"))]
fn activate_macos_application() {}
