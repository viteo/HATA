//! Taps from a touchscreen on the Linux console, where the terminal doesn't report the mouse.
//! A tap is turned into a character cell.

#[cfg(target_os = "linux")]
pub use linux::Touchscreen;

#[cfg(target_os = "linux")]
mod linux {
    use evdev::{AbsInfo, AbsoluteAxisCode, Device, EventStream, EventSummary, KeyCode, PropType, SynchronizationCode};
    use std::{fs, path::Path};

    pub struct Touchscreen {
        events: EventStream,
        screen: Screen,
        x: i32,
        y: i32,
        lifted: bool,
    }

    impl Touchscreen {
        /// Reading needs access to /dev/input, usually the `input` group.
        pub fn open() -> Option<Self> {
            let font = console_font()?;
            let (_, device) = evdev::enumerate().find(|(_, device)| is_touchscreen(device))?;
            let screen = Screen::new(&device, font)?;
            let events = device.into_event_stream().ok()?;
            Some(Touchscreen { events, screen, x: 0, y: 0, lifted: false })
        }

        /// The (column, row) of the console where the next tap is.
        pub async fn next_tap(&mut self) -> Option<(u16, u16)> {
            loop {
                match self.events.next_event().await.ok()?.destructure() {
                    EventSummary::AbsoluteAxis(_, AbsoluteAxisCode::ABS_X, value) => self.x = value,
                    EventSummary::AbsoluteAxis(_, AbsoluteAxisCode::ABS_Y, value) => self.y = value,
                    EventSummary::Key(_, KeyCode::BTN_TOUCH, 0) => self.lifted = true,
                    // the position is complete at the end of the report
                    EventSummary::Synchronization(_, SynchronizationCode::SYN_REPORT, _) if self.lifted => {
                        self.lifted = false;
                        if is_active_console() {
                            return Some(self.screen.cell(self.x, self.y));
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    /// A screen touched directly (not a touchpad) that reports where it is touched.
    fn is_touchscreen(device: &Device) -> bool {
        device.properties().contains(PropType::DIRECT)
            && device.supported_keys().is_some_and(|keys| keys.contains(KeyCode::BTN_TOUCH))
            && device
                .supported_absolute_axes()
                .is_some_and(|axes| axes.contains(AbsoluteAxisCode::ABS_X) && axes.contains(AbsoluteAxisCode::ABS_Y))
    }

    /// How touchscreen positions map to the console.
    struct Screen {
        x: AbsInfo,
        y: AbsInfo,
        /// framebuffer size in pixels, as the panel is mounted
        width: i32,
        height: i32,
        /// quarter turns of the console, see https://docs.kernel.org/fb/fbcon.html
        rotate: u8,
        font: (i32, i32),
    }

    impl Screen {
        fn new(device: &Device, font: (i32, i32)) -> Option<Self> {
            let axis = |code| device.get_absinfo().ok()?.find(|(c, _)| *c == code).map(|(_, info)| info);
            let size = fs::read_to_string("/sys/class/graphics/fb0/virtual_size").ok()?;
            let (width, height) = size.trim().split_once(',')?;
            let rotate = fs::read_to_string("/sys/class/graphics/fbcon/rotate").ok().and_then(|r| r.trim().parse().ok());
            Some(Screen {
                x: axis(AbsoluteAxisCode::ABS_X)?,
                y: axis(AbsoluteAxisCode::ABS_Y)?,
                width: width.parse().ok()?,
                height: height.parse().ok()?,
                rotate: rotate.unwrap_or(0),
                font,
            })
        }

        fn cell(&self, x: i32, y: i32) -> (u16, u16) {
            let scale = |value: i32, axis: &AbsInfo, pixels: i32| {
                let range = (axis.maximum() - axis.minimum()).max(1);
                ((value - axis.minimum()) * (pixels - 1) / range).clamp(0, pixels - 1)
            };
            let (x, y) = (scale(x, &self.x, self.width), scale(y, &self.y, self.height));
            let (x, y) = match self.rotate {
                1 => (y, self.width - 1 - x),
                2 => (self.width - 1 - x, self.height - 1 - y),
                3 => (self.height - 1 - y, x),
                _ => (x, y),
            };
            ((x / self.font.0) as u16, (y / self.font.1) as u16)
        }
    }

    /// Width and height of the console font, when the terminal is a Linux console.
    fn console_font() -> Option<(i32, i32)> {
        // KDFONTOP and struct console_font_op from linux/kd.h
        #[repr(C)]
        struct ConsoleFontOp {
            op: u32,
            flags: u32,
            width: u32,
            height: u32,
            charcount: u32,
            data: *mut u8,
        }
        const KDFONTOP: u32 = 0x4B72;
        const KD_FONT_OP_GET: u32 = 1;

        let mut font = ConsoleFontOp { op: KD_FONT_OP_GET, flags: 0, width: u32::MAX, height: u32::MAX, charcount: 0, data: std::ptr::null_mut() };
        let result = unsafe { libc::ioctl(libc::STDIN_FILENO, KDFONTOP as _, &mut font) };
        (result == 0 && font.width > 0 && font.height > 0).then_some((font.width as i32, font.height as i32))
    }

    /// Whether HATA's console is the one on the screen, as the touchscreen is shared by all of them.
    fn is_active_console() -> bool {
        match (fs::read_to_string("/sys/class/tty/tty0/active"), fs::read_link("/proc/self/fd/0")) {
            (Ok(active), Ok(ours)) => ours == Path::new("/dev").join(active.trim()),
            _ => true,
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        fn screen(width: i32, height: i32, rotate: u8) -> Screen {
            let axis = |pixels: i32| AbsInfo::new(0, 0, pixels - 1, 0, 0, 0);
            Screen { x: axis(width), y: axis(height), width, height, rotate, font: (16, 32) }
        }

        #[test]
        fn taps_land_in_the_corners_of_an_upright_console() {
            let screen = screen(1080, 2246, 0);
            assert_eq!(screen.cell(55, 49), (3, 1));
            assert_eq!(screen.cell(1009, 58), (63, 1));
            assert_eq!(screen.cell(46, 1442), (2, 45));
        }

        #[test]
        fn taps_land_in_the_corners_of_a_rotated_console() {
            let screen = screen(1080, 2280, 3);
            assert_eq!(screen.cell(56, 2183), (6, 1));
            assert_eq!(screen.cell(64, 98), (136, 2));
            assert_eq!(screen.cell(474, 2260), (1, 14));
        }

        #[test]
        fn touch_range_is_scaled_to_the_screen() {
            let mut screen = screen(1080, 2280, 0);
            screen.x = AbsInfo::new(0, 0, 4095, 0, 0, 0);
            assert_eq!(screen.cell(4095, 0), (67, 0));
        }
    }
}

/// Non linux dummy
#[cfg(not(target_os = "linux"))]
pub struct Touchscreen;

#[cfg(not(target_os = "linux"))]
impl Touchscreen {
    pub fn open() -> Option<Self> {
        None
    }

    pub async fn next_tap(&mut self) -> Option<(u16, u16)> {
        None
    }
}
