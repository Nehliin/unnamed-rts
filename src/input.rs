use legion::*;
use std::{collections::HashSet, hash::Hash};
use winit::{dpi::PhysicalPosition, event::{ModifiersState, MouseButton, MouseScrollDelta, *, self}};

// EventReader? store event at frame start and clear at frame end?
// Input<KeyboardInput> input <MouseMotion> Input <MouseButton>
// input<MouseWheel>
// ResizeEvent
//
#[derive(Debug, Default)]
pub struct Text {
    pub codepoint: char,
}
#[derive(Debug, Clone, Copy)]
pub struct MouseMotion {
    pub delta_x: f64,
    pub delta_y: f64,
}

#[derive(Debug, Default)]
pub struct CursorPosition {
    pub x: f64,
    pub y: f64
}

#[derive(Debug)]
pub struct FrameEvent<T> {
    pub event: Option<T>,
}

impl<T> FrameEvent<T> {
    pub fn new(event: T) -> Self {
        FrameEvent { event: Some(event) }
    }
}

impl<T> Default for FrameEvent<T> {
    fn default() -> Self {
        Self {
            event: None
        }
    }
}

#[system]
pub fn input(
    #[resource] text_input: &mut FrameEvent<Text>,
    #[resource] mouse_scroll: &mut FrameEvent<MouseScrollDelta>,
    #[resource] mouse_motion: &mut FrameEvent<MouseMotion>,
    #[resource] modifiers_state: &mut FrameEvent<ModifiersState>,
    #[resource] keyboard_state: &mut KeyboardState,
    #[resource] mousebutton_state: &mut MouseButtonState,
) {
    keyboard_state.frame_update();
    mousebutton_state.pressed_current_frame.clear();
    mousebutton_state.released_current_frame.clear();

    mouse_scroll.event = None;
    mouse_motion.event = None;
    text_input.event = None;
    modifiers_state.event = None;
}

#[derive(Default, Debug)]
struct BitSet {
    primary: u128,
    secondary: u64,
}

// Yes this logic is quite unreadable but bitricks are fun :)
// and this is a project done for fun
impl BitSet {
    // starting at 0
    fn set_bit(&mut self, bit: u32) {
        debug_assert!(bit <= (128 + 64));
        if bit < 128 {
            self.primary |= 1 << bit;
        } else {
            // 128 -> 192
            self.secondary |= 1 << (bit - 128);
        }
    }

    fn unset_bit(&mut self, bit: u32) {
        debug_assert!(bit <= (128 + 64));
        if bit < 128 {
            self.primary ^= 1 << bit;
        } else {
            // 128 -> 192
            self.secondary ^= 1 << (bit - 128);
        }
    }

    fn is_set(&self, bit: u32) -> bool {
        debug_assert!(bit <= (128 + 64));
        if bit < 128 {
            (self.primary & 1 << bit) != 0
        } else {
            // 128 -> 192
            (self.secondary & 1 << (bit - 128)) != 0
        }
    }

    fn clear(&mut self) {
        self.primary = 0;
        self.secondary = 0;
    }
}
#[derive(Debug, Default)]
pub struct MouseButtonState {
    pub pressed: HashSet<MouseButton>,
    pub pressed_current_frame: HashSet<MouseButton>,
    pub released_current_frame: HashSet<MouseButton>,
}
/*
impl MouseButtonState {
    pub fn pressed(&self, button: &MouseButton) -> bool {
        self.pressed.contains(button)
    }

    pub fn pressed_current_frame(&self, button: &MouseButton) -> bool {
        self.pressed_current_frame.contains(button)
    }

    pub fn released_current_frame(&self, button: &MouseButton) -> bool {
        self.released_current_frame.contains(button)
    }

    pub
}*/

#[derive(Debug, Default)]
pub struct KeyboardState {
    pressed: BitSet,
    pressed_current_frame: BitSet,
    released_current_frame: BitSet,
    // modifiers?
}

impl KeyboardState {
    pub fn set_pressed(&mut self, key: VirtualKeyCode) {
        self.pressed.set_bit(key as u32);
        self.pressed_current_frame.set_bit(key as u32);
    }

    pub fn set_released(&mut self, key: VirtualKeyCode) {
        debug_assert!(self.pressed.is_set(key as u32));
        self.pressed.unset_bit(key as u32);
        self.pressed_current_frame.unset_bit(key as u32);
        self.released_current_frame.set_bit(key as u32);
    }

    pub fn frame_update(&mut self) {
        self.pressed_current_frame.clear();
        self.released_current_frame.clear();
    }

    pub fn is_pressed(&self, key: VirtualKeyCode) -> bool {
        self.pressed.is_set(key as u32)
    }

    pub fn pressed_current_frame(&self, key: VirtualKeyCode) -> bool {
        self.pressed_current_frame.is_set(key as u32)
    }

    pub fn released_current_frame(&self, key: VirtualKeyCode) -> bool {
        self.released_current_frame.is_set(key as u32)
    }

    pub fn all_pressed(&self) -> HashSet<VirtualKeyCode> {
        Self::convert_to_virtual_keyset(&self.pressed)
    }

    pub fn all_pressed_current_frame(&self) -> HashSet<VirtualKeyCode> {
        Self::convert_to_virtual_keyset(&self.pressed_current_frame)
    }

    pub fn all_release_current_frame(&self) -> HashSet<VirtualKeyCode> {
        Self::convert_to_virtual_keyset(&self.released_current_frame)
    }
    // unclear if this even faster than just using allocating some hashsets...
    #[inline]
    fn convert_to_virtual_keyset(storage: &BitSet) -> HashSet<VirtualKeyCode> {
        let mut result = HashSet::with_capacity(32);
        for bit in 0..(128 + 64) {
            if storage.is_set(bit) {
                // SAFETY: Since the fields are private the only modification should have been made
                // by set_pressed or simlilar meaning the code must be a valid enum discriminant
                // I know these are unecessary optimisations compared to storing in a HashSet but
                // getting rid of allocations + bittwiddling is fun for something that isn't in prod :)
                result.insert(unsafe { std::mem::transmute(bit) });
            }
        }
        result
    }
}
use winit::event::VirtualKeyCode::*;
#[inline]
pub fn winit_to_egui_key_code(key: VirtualKeyCode) -> Option<egui::Key> {
    Some(match key {
        Escape => egui::Key::Escape,
        Insert => egui::Key::Insert,
        Home => egui::Key::Home,
        Delete => egui::Key::Delete,
        End => egui::Key::End,
        PageDown => egui::Key::PageDown,
        PageUp => egui::Key::PageUp,
        Left => egui::Key::ArrowLeft,
        Up => egui::Key::ArrowUp,
        Right => egui::Key::ArrowRight,
        Down => egui::Key::ArrowDown,
        Back => egui::Key::Backspace,
        Return => egui::Key::Enter,
        Tab => egui::Key::Tab,
        Space => egui::Key::Space,

        A => egui::Key::A,
        B => egui::Key::B,
        C => egui::Key::C,
        D => egui::Key::D,
        E => egui::Key::E,
        F => egui::Key::F,
        G => egui::Key::G,
        H => egui::Key::H,
        I => egui::Key::I,
        J => egui::Key::J,
        K => egui::Key::K,
        L => egui::Key::L,
        M => egui::Key::M,
        N => egui::Key::N,
        O => egui::Key::O,
        P => egui::Key::P,
        Q => egui::Key::Q,
        R => egui::Key::R,
        S => egui::Key::S,
        T => egui::Key::T,
        U => egui::Key::U,
        V => egui::Key::V,
        W => egui::Key::W,
        X => egui::Key::X,
        Y => egui::Key::Y,
        Z => egui::Key::Z,
        _ => {
            return None;
        }
    })
}

/// Translates winit to egui modifier keys.
#[inline]
pub fn winit_to_egui_modifiers(modifiers: ModifiersState) -> egui::Modifiers {
    egui::Modifiers {
        alt: modifiers.alt(),
        ctrl: modifiers.ctrl(),
        shift: modifiers.shift(),
        #[cfg(target_os = "macos")]
        mac_cmd: modifiers.logo(),
        #[cfg(target_os = "macos")]
        command: modifiers.logo(),
        #[cfg(not(target_os = "macos"))]
        mac_cmd: false,
        #[cfg(not(target_os = "macos"))]
        command: modifiers.ctrl(),
    }
}
#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_insertion() {
        let mut keystate = KeyboardState::default();
        keystate.set_pressed(VirtualKeyCode::A);
        keystate.set_pressed(VirtualKeyCode::A);
        keystate.set_pressed(VirtualKeyCode::Cut);
        assert!(keystate.is_pressed(VirtualKeyCode::A));
        assert!(keystate.is_pressed(VirtualKeyCode::Cut));
        assert!(keystate.pressed_current_frame(VirtualKeyCode::Cut));
        assert!(keystate.pressed_current_frame(VirtualKeyCode::A));

        let all = keystate.all_pressed();
        assert!(all.len() == 2);
        assert!(all.contains(&VirtualKeyCode::A));
        assert!(all.contains(&VirtualKeyCode::Cut));
        let current_frame = keystate.all_pressed();
        assert!(current_frame.len() == 2);
        assert!(current_frame.contains(&VirtualKeyCode::A));
        assert!(current_frame.contains(&VirtualKeyCode::Cut));
    }

    #[test]
    fn test_removal() {
        let mut keystate = KeyboardState::default();
        keystate.set_pressed(VirtualKeyCode::A);
        keystate.set_pressed(VirtualKeyCode::A);
        keystate.set_pressed(VirtualKeyCode::Cut);

        keystate.set_released(VirtualKeyCode::A);
        assert!(!keystate.is_pressed(VirtualKeyCode::A));
        assert!(!keystate.all_pressed().contains(&VirtualKeyCode::A));
        assert!(keystate.all_pressed().contains(&VirtualKeyCode::Cut));
        assert!(keystate
            .all_release_current_frame()
            .contains(&VirtualKeyCode::A))
    }

    #[test]
    fn test_update() {
        let mut keystate = KeyboardState::default();
        keystate.set_pressed(VirtualKeyCode::A);
        keystate.set_pressed(VirtualKeyCode::A);
        keystate.set_pressed(VirtualKeyCode::Cut);

        keystate.frame_update();

        assert!(keystate.is_pressed(VirtualKeyCode::A));
        assert!(keystate.is_pressed(VirtualKeyCode::Cut));
        assert!(keystate.all_pressed().contains(&VirtualKeyCode::Cut));
        assert!(keystate.all_pressed().contains(&VirtualKeyCode::A));
        assert!(!keystate
            .all_pressed_current_frame()
            .contains(&VirtualKeyCode::Cut));
        assert!(!keystate
            .all_pressed_current_frame()
            .contains(&VirtualKeyCode::A));

        keystate.set_released(VirtualKeyCode::A);

        assert!(!keystate.is_pressed(VirtualKeyCode::A));
        assert!(keystate
            .all_release_current_frame()
            .contains(&VirtualKeyCode::A));

        keystate.frame_update();
        assert!(!keystate.is_pressed(VirtualKeyCode::A));
        assert!(!keystate
            .all_release_current_frame()
            .contains(&VirtualKeyCode::A));
    }
}
