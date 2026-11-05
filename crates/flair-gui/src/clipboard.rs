use winit::raw_window_handle::RawDisplayHandle;

pub struct Clipboard(Option<smithay_clipboard::Clipboard>);
impl Clipboard {
    pub fn new(display: RawDisplayHandle) -> Self {
        let clipboard = match display {
            RawDisplayHandle::Wayland(handle) => {
                Some(unsafe { smithay_clipboard::Clipboard::new(handle.display.as_ptr()) })
            }
            _ => None,
        };
        Self(clipboard)
    }
    pub fn load(&self) -> Option<String> {
        self.0.as_ref()?.load().ok()
    }
}
