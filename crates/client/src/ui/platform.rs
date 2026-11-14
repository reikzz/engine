use slint::platform::software_renderer::MinimalSoftwareWindow;
use slint::platform::{Platform, WindowAdapter};
use std::rc::Rc;

pub struct CustomSlintPlatform {
    window: Rc<MinimalSoftwareWindow>,
}

impl CustomSlintPlatform {
    pub fn new(window: Rc<MinimalSoftwareWindow>) -> Self {
        Self { window }
    }
}

impl Platform for CustomSlintPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        Ok(self.window.clone())
    }
}
