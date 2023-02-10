use windows::{
        core::*,
        Win32::Foundation::*,
        Win32::System::LibraryLoader::*,
        Win32::UI::WindowsAndMessaging::*,
        Win32::UI::Input::KeyboardAndMouse::*,
        Win32::UI::Controls::*,
};

use core::mem::size_of;
use core::mem::transmute;

#[allow(dead_code)]
#[derive(Debug)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
}

#[allow(dead_code)]
#[derive(Debug)]
pub enum Event {
    Quit,
    KeyPress(Option<char>),
    KeyRelease(Option<char>),
    MouseMove(i32, i32),
    MousePress(MouseButton),
    MouseRelease(MouseButton),
    MouseLeave,
    MouseWheel(f32, f32),
    Focus(bool),
    Resize(u32, u32),
    Minimized,
}

struct WindowState {
    width: u32,
    height: u32,
    mouse_tracked: bool,
    mouse_button_mask: u8,
    resized: bool,
}

pub struct Window {
    pub handle: HWND,
    state: Box<WindowState>,
}

impl Window {
    pub fn width(&self) -> u32 {
        self.state.width
    }

    pub fn height(&self) -> u32 {
        self.state.height
    }

    pub fn poll_events(&mut self) -> Option<Event> {
        let mut message = MSG::default();

        // Handle resize events coming from the window proc, we don't get a
        // message for those
        if self.state.resized {
            self.state.resized = false;
            if self.state.width == 0 || self.state.height == 0 {
                return Some(Event::Minimized);
            } else {
                return Some(Event::Resize(self.state.width, self.state.height));
            }
        }


        if unsafe { PeekMessageA(&mut message, None, 0, 0, PM_REMOVE) }.into() {
            unsafe {
                TranslateMessage(&message);
                DispatchMessageA(&message);
            }

            match message.message {
                WM_QUIT => Some(Event::Quit),
                WM_KEYDOWN => {
                    let key = char::from_u32(message.wParam.0 as u32);
                    Some(Event::KeyPress(key))
                }

                WM_KEYUP => {
                    let key = char::from_u32(message.wParam.0 as u32);
                    Some(Event::KeyRelease(key))
                }

                WM_MOUSEMOVE => {
                    if !self.state.mouse_tracked {
                        let mut tme = TRACKMOUSEEVENT {
                            cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                            dwFlags: TME_LEAVE,
                            hwndTrack: self.handle,
                            dwHoverTime: 0,
                        };
                        unsafe {
                            if TrackMouseEvent(&mut tme).0 != 0{
                                self.state.mouse_tracked = true;
                            }
                        }
                    }
                    let x = (message.lParam.0 & 0xFFFF) as i16 as i32;
                    let y = (message.lParam.0 >> 16) as i16 as i32;
                    Some(Event::MouseMove(x, y))
                }

                WM_MOUSELEAVE => {
                    self.state.mouse_tracked = false;
                    Some(Event::MouseLeave)
                }

                WM_LBUTTONDOWN | WM_LBUTTONDBLCLK |
                WM_RBUTTONDOWN | WM_RBUTTONDBLCLK |
                WM_MBUTTONDOWN | WM_MBUTTONDBLCLK => {
                    if self.state.mouse_button_mask == 0 &&
                        unsafe { GetCapture().0 } == 0 {

                        unsafe { SetCapture(self.handle); }
                    }

                    let (idx, button) = match message.message {
                        WM_LBUTTONDOWN | WM_LBUTTONDBLCLK =>
                            (0, MouseButton::Left),
                        WM_RBUTTONDOWN | WM_RBUTTONDBLCLK =>
                            (1, MouseButton::Right),
                        WM_MBUTTONDOWN | WM_MBUTTONDBLCLK =>
                            (2, MouseButton::Middle),
                        _ => unreachable!(),
                    };

                    self.state.mouse_button_mask |= 1 << idx;
                    Some(Event::MousePress(button))
                }

                WM_LBUTTONUP |
                WM_RBUTTONUP |
                WM_MBUTTONUP => {
                    let (idx, button) = match message.message {
                        WM_LBUTTONUP => (0, MouseButton::Left),
                        WM_RBUTTONUP => (1, MouseButton::Right),
                        WM_MBUTTONUP => (2, MouseButton::Middle),
                        _ => unreachable!(),
                    };

                    self.state.mouse_button_mask &= !(1 << idx);
                    if self.state.mouse_button_mask == 0 &&
                        unsafe { GetCapture() } == self.handle {
                        unsafe { ReleaseCapture(); }
                    }

                    Some(Event::MouseRelease(button))
                }

                WM_MOUSEWHEEL => {
                    let delta = (message.wParam.0 >> 16) as i16 as f32
                        / WHEEL_DELTA as f32;
                    Some(Event::MouseWheel(0.0, delta))
                }

                WM_MOUSEHWHEEL => {
                    let delta = (message.wParam.0 >> 16) as i16 as f32
                        / WHEEL_DELTA as f32;
                    Some(Event::MouseWheel(delta, 0.0))
                }

                WM_SETFOCUS => Some(Event::Focus(true)),
                WM_KILLFOCUS => Some(Event::Focus(false)),

                _ => None
            }
        } else {
            None
        }

    }

}

extern "system" fn wndproc(window: HWND, message: u32, wparam: WPARAM, lparam:
                           LPARAM) -> LRESULT {
    let mut state: Option<&mut WindowState> = unsafe {
        let param = GetWindowLongPtrA(window, GWLP_USERDATA) as *mut WindowState;
        if param.is_null() {
            None
        } else {
            Some(&mut *param)
        }
    };

    match message {
        WM_CREATE => {
            unsafe {
                let create_struct: &CREATESTRUCTA = transmute(lparam);
                SetWindowLongPtrA(window, GWLP_USERDATA,
                               create_struct.lpCreateParams as _);
            }
            LRESULT::default()
        }
        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            LRESULT::default()
        }

        WM_KEYDOWN => {
            if wparam.0 == VK_ESCAPE.0 as usize {
               unsafe { DestroyWindow(window); }
            }
            LRESULT::default()
        }

        WM_SIZE => {
            let (width, height) =
                if wparam.0 == SIZE_MINIMIZED as usize {
                    (0, 0)
                } else {
                    ((lparam.0 & 0xFFFF) as u32,
                     (lparam.0 >> 16) as u32)
                };

            if let Some(state) = state.as_mut() {
                if state.width != width || state.height != height {
                    state.resized = true;
                    state.width  = width;
                    state.height = height;
                }
            }

            LRESULT::default()
        }


        _ => {
            unsafe { DefWindowProcA(window, message, wparam, lparam) }
        }
    }
}


pub fn create_window(title: &str, width: u32, height: u32) -> Option<Window> {
    let instance = unsafe { GetModuleHandleA(None).ok()? };

    let wc = WNDCLASSEXA {
        cbSize: size_of::<WNDCLASSEXA>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(wndproc),
        hInstance: instance,
        hCursor: unsafe { LoadCursorW(None, IDC_ARROW).ok()? },
        lpszClassName: PCSTR(b"window_class\0".as_ptr()),
        ..Default::default()
    };

    if unsafe { RegisterClassExA(&wc) } == 0 {
        return None;
    }

    let mut window_rect = RECT {
        left: 0, top: 0,
        right: width.try_into().ok()?,
        bottom: height.try_into().ok()?,
    };

    unsafe { AdjustWindowRect(&mut window_rect, WS_OVERLAPPEDWINDOW, false) };

    let mut state = Box::new(WindowState {
        width,
        height,
        mouse_tracked: false,
        mouse_button_mask: 0,
        resized: false,
    });

    let handle = unsafe {
        let param = &mut *state as *mut WindowState as *mut _;

        CreateWindowExA(
            Default::default(),
            "window_class",
            title,
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            window_rect.right - window_rect.left,
            window_rect.bottom - window_rect.top,
            None, // no parent window
            None, // no menus
            instance,
            param
        )
    };

    unsafe { ShowWindow(handle, SW_SHOW) };

    Some(Window{
        handle,
        state,
    })
}
