use egui::{CtxRef, Key, RawInput, paint::ClippedShape, pos2, vec2};
use winit::event::VirtualKeyCode::*;
use winit::event::WindowEvent::*;
use winit::event::{Event, ModifiersState, VirtualKeyCode};

#[derive(Debug)]
pub struct WindowSize {
    pub physical_width: u32,
    pub physical_height: u32,
    pub scale_factor: f32,
}

impl WindowSize {
    pub fn logical_size(&self) -> (u32, u32) {
        let logical_width = self.physical_width as f32 / self.scale_factor;
        let logical_height = self.physical_height as f32 / self.scale_factor;
        (logical_width as u32, logical_height as u32)
    }
}

pub struct UiContext {
    pub context: CtxRef,
    raw_input: RawInput,
    modifier_state: ModifiersState, // not needed??
}

impl UiContext {
    pub fn new(window_size: &WindowSize) -> UiContext {
        let context = CtxRef::default();
        let raw_input = egui::RawInput {
            pixels_per_point: Some(window_size.scale_factor),
            screen_rect: Some(egui::Rect::from_min_size(
                Default::default(),
                vec2(
                    window_size.physical_width as f32,
                    window_size.physical_height as f32,
                ) / window_size.scale_factor,
            )),
            ..Default::default()
        };

        UiContext {
            context,
            raw_input,
            modifier_state: ModifiersState::empty(),
        }
    }

    pub fn update_time(&mut self, elapsed_seconds: f64) {
        self.raw_input.time = Some(elapsed_seconds);
    }

    pub fn begin_frame(&mut self) {
        self.context.begin_frame(self.raw_input.take());
    }

    pub fn end_frame(&mut self) -> (egui::Output, Vec<ClippedShape>) {
        self.context.end_frame()
    }
}

// This should be a system
pub fn handle_input<T>(context: &mut UiContext, window_size: &mut WindowSize, event: &Event<T>) {
    match event {
        Event::WindowEvent {
            window_id: _window_id,
            event,
        } => match event {
            Resized(physical_size) => {
                window_size.physical_width = physical_size.width;
                window_size.physical_height = physical_size.height;
                // break this out?
                context.raw_input.screen_rect = Some(egui::Rect::from_min_size(
                    Default::default(),
                    vec2(
                        window_size.physical_width as f32,
                        window_size.physical_height as f32,
                    ) / window_size.scale_factor as f32,
                ));
            }
            ScaleFactorChanged {
                scale_factor,
                new_inner_size,
            } => {
                window_size.scale_factor = *scale_factor as f32;
                context.raw_input.pixels_per_point = Some(*scale_factor as f32);
                context.raw_input.screen_rect = Some(egui::Rect::from_min_size(
                    Default::default(),
                    vec2(new_inner_size.width as f32, new_inner_size.height as f32)
                        / window_size.scale_factor as f32,
                ));
            }
            MouseInput { state, .. } => {
                context.raw_input.mouse_down = *state == winit::event::ElementState::Pressed;
            }
            MouseWheel { delta, .. } => {
                match delta {
                    winit::event::MouseScrollDelta::LineDelta(x, y) => {
                        let line_height = 24.0; // TODO as in egui_glium
                        context.raw_input.scroll_delta = vec2(*x, *y) * line_height;
                    }
                    winit::event::MouseScrollDelta::PixelDelta(delta) => {
                        // Actually point delta
                        context.raw_input.scroll_delta = vec2(delta.x as f32, delta.y as f32);
                    }
                }
            }
            CursorMoved { position, .. } => {
                context.raw_input.mouse_pos = Some(pos2(
                    position.x as f32 / context.raw_input.pixels_per_point.unwrap(),
                    position.y as f32 / context.raw_input.pixels_per_point.unwrap(),
                ));
            }
            CursorLeft { .. } => {
                context.raw_input.mouse_pos = None;
            }
            ModifiersChanged(input) => context.modifier_state = *input,
            KeyboardInput { input, .. } => {
                if let Some(virtual_keycode) = input.virtual_keycode {
                    let pressed = input.state == winit::event::ElementState::Pressed;

                    if pressed {
                        if let Some(key) = winit_to_egui_key_code(virtual_keycode) {
                            context.raw_input.events.push(egui::Event::Key {
                                key,
                                pressed: input.state == winit::event::ElementState::Pressed,
                                modifiers: winit_to_egui_modifiers(context.modifier_state),
                            });
                        }
                    }
                }
            }
            ReceivedCharacter(ch) => {
                if is_printable(*ch)
                    && !context.modifier_state.ctrl()
                    && !context.modifier_state.logo()
                {
                    context
                        .raw_input
                        .events
                        .push(egui::Event::Text(ch.to_string()));
                }
            }
            _ => {}
        },
        Event::DeviceEvent { .. } => {}
        _ => {}
    }
}

/// Translates winit to egui keycodes.
#[inline]
fn winit_to_egui_key_code(key: VirtualKeyCode) -> Option<egui::Key> {
    Some(match key {
        Escape => Key::Escape,
        Insert => Key::Insert,
        Home => Key::Home,
        Delete => Key::Delete,
        End => Key::End,
        PageDown => Key::PageDown,
        PageUp => Key::PageUp,
        Left => Key::ArrowLeft,
        Up => Key::ArrowUp,
        Right => Key::ArrowRight,
        Down => Key::ArrowDown,
        Back => Key::Backspace,
        Return => Key::Enter,
        Tab => Key::Tab,
        Space => Key::Space,

        A => Key::A,
        K => Key::K,
        U => Key::U,
        W => Key::W,
        Z => Key::Z,

        _ => {
            return None;
        }
    })
}

/// Translates winit to egui modifier keys.
#[inline]
fn winit_to_egui_modifiers(modifiers: ModifiersState) -> egui::Modifiers {
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

/// We only want printable characters and ignore all special keys.
#[inline]
fn is_printable(chr: char) -> bool {
    let is_in_private_use_area = ('\u{e000}'..='\u{f8ff}').contains(&chr)
        || ('\u{f0000}'..='\u{ffffd}').contains(&chr)
        || ('\u{100000}'..='\u{10fffd}').contains(&chr);

    !is_in_private_use_area && !chr.is_ascii_control()
}
