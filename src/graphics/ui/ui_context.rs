use egui::{paint::ClippedShape, pos2, vec2, CtxRef, Key, RawInput};
use legion::Resources;
use winit::event::{Event, ModifiersState, VirtualKeyCode};

use crate::input::{self, FrameEvent, winit_to_egui_key_code, winit_to_egui_modifiers};

#[derive(Debug, Clone, Copy)]
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

pub struct CursorPosition {
    pub x: f64,
    pub y: f64,
}

pub struct UiContext {
    pub context: CtxRef,
    pub raw_input: RawInput,
    pub cursor_pos: CursorPosition,
    pub modifier_state: ModifiersState, // not needed??
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
            cursor_pos: CursorPosition { x: 0.0, y: 0.0 },
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
 use winit::event::WindowEvent::*;

use super::ui_systems::is_printable;
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
