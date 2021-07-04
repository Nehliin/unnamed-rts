use std::time::Instant;

use crate::{
    assets::Assets,
    input::MouseMotion,
    rendering::pass::ui_pass::UiPass,
    resources::{Time, WindowSize},
};
use egui::{pos2, vec2, Pos2};
use input::{CursorPosition, Text};
use legion::*;
use wgpu::{CommandEncoderDescriptor, Device, Queue, SwapChainTexture};
use winit::event::{ModifiersState, MouseButton, MouseScrollDelta};

use crate::input::{self, EventReader};

use super::ui_resources::{UiContext, UiTexture};

fn handle_mouse_input(
    mouse_input: &input::MouseButtonState,
    mouse_button: &MouseButton,
    mapped_button: egui::PointerButton,
    current_cursor_pos: Pos2,
    ui_ctx: &mut UiContext,
) {
    if mouse_input.pressed_current_frame(mouse_button) {
        ui_ctx.raw_input.events.push(egui::Event::PointerButton {
            pos: current_cursor_pos,
            button: mapped_button,
            pressed: true,
            modifiers: Default::default(),
        });
    } else if mouse_input.released_current_frame(mouse_button) {
        ui_ctx.raw_input.events.push(egui::Event::PointerButton {
            pos: current_cursor_pos,
            button: mapped_button,
            pressed: false,
            modifiers: Default::default(),
        });
    }
}

#[allow(clippy::too_many_arguments)]
#[system]
pub fn update_ui(
    #[resource] ui_ctx: &mut UiContext,
    #[resource] window_size: &WindowSize,
    #[resource] modifiers_changed: &EventReader<ModifiersState>,
    #[resource] mouse_position: &CursorPosition,
    #[resource] mouse_scroll: &EventReader<MouseScrollDelta>,
    #[resource] mouse_motion: &EventReader<MouseMotion>,
    #[resource] text_input: &EventReader<Text>,
    #[resource] mouse_input: &input::MouseButtonState,
    #[resource] key_input: &input::KeyboardState,
) {
    ui_ctx.raw_input.pixels_per_point = Some(window_size.scale_factor);
    ui_ctx.raw_input.screen_rect = Some(egui::Rect::from_min_max(
        pos2(0.0, 0.0),
        pos2(
            window_size.physical_width as f32 / window_size.scale_factor as f32,
            window_size.physical_height as f32 / window_size.scale_factor as f32,
        ),
    ));

    // Keep in mind that the cursor left event isn't handled
    let current_cursor_pos = pos2(
        mouse_position.x as f32 / ui_ctx.raw_input.pixels_per_point.unwrap(),
        mouse_position.y as f32 / ui_ctx.raw_input.pixels_per_point.unwrap(),
    );
    if mouse_motion.last_event().is_some() {
        ui_ctx.raw_input.events.push(egui::Event::PointerMoved(pos2(
            current_cursor_pos.x,
            current_cursor_pos.y,
        )));
    }
    // Handle mouse input
    // TODO: This will cause the ui to not capture mouse clicks, they will still be registered by systems "behind" the ui which is undesirable
    handle_mouse_input(
        mouse_input,
        &MouseButton::Left,
        egui::PointerButton::Primary,
        current_cursor_pos,
        ui_ctx,
    );
    handle_mouse_input(
        mouse_input,
        &MouseButton::Right,
        egui::PointerButton::Secondary,
        current_cursor_pos,
        ui_ctx,
    );
    handle_mouse_input(
        mouse_input,
        &MouseButton::Middle,
        egui::PointerButton::Middle,
        current_cursor_pos,
        ui_ctx,
    );

    for scroll_delta in mouse_scroll.events() {
        match scroll_delta {
            MouseScrollDelta::LineDelta(x, y) => {
                ui_ctx.raw_input.scroll_delta += vec2(*x, *y);
            }
            MouseScrollDelta::PixelDelta(delta) => {
                // Actually point delta
                ui_ctx.raw_input.scroll_delta += vec2(delta.x as f32, delta.y as f32);
            }
        }
    }

    if let Some(modifier_state) = modifiers_changed.last_event() {
        ui_ctx.modifier_state = *modifier_state;
    }
    ui_ctx.raw_input.modifiers = input::winit_to_egui_modifiers(ui_ctx.modifier_state);

    for text in text_input.events() {
        if is_printable(text.codepoint)
            && !ui_ctx.modifier_state.ctrl()
            && !ui_ctx.modifier_state.logo()
        {
            ui_ctx
                .raw_input
                .events
                .push(egui::Event::Text(text.codepoint.to_string()));
        }
    }

    for key in key_input.all_pressed_current_frame() {
        if let Some(key) = input::winit_to_egui_key_code(key) {
            ui_ctx.raw_input.events.push(egui::Event::Key {
                key,
                pressed: true,
                modifiers: ui_ctx.raw_input.modifiers,
            })
        }
    }

    for key in key_input.all_release_current_frame() {
        if let Some(key) = input::winit_to_egui_key_code(key) {
            ui_ctx.raw_input.events.push(egui::Event::Key {
                key,
                pressed: false,
                modifiers: ui_ctx.raw_input.modifiers,
            })
        }
    }
}

/// We only want printable characters and ignore all special keys.
#[inline]
pub fn is_printable(chr: char) -> bool {
    let is_in_private_use_area = ('\u{e000}'..='\u{f8ff}').contains(&chr)
        || ('\u{f0000}'..='\u{ffffd}').contains(&chr)
        || ('\u{100000}'..='\u{10fffd}').contains(&chr);

    !is_in_private_use_area && !chr.is_ascii_control()
}

#[system]
pub fn begin_ui_frame(
    #[state] time_since_start: &Instant,
    #[resource] time: &Time,
    #[resource] ui_context: &mut UiContext,
) {
    ui_context.raw_input.time = Some(time_since_start.elapsed().as_secs_f64());
    ui_context.raw_input.predicted_dt = time.delta_time;
    ui_context.context.begin_frame(ui_context.raw_input.take());
}

// TODO: handle user textures here
// Basicall simply load texture data to create a egui::Texture and then run egui texture to wgpu texture
// However, better to map texture id = already loaded texture (via Aseets<>) and handle it from there
#[system]
pub fn end_ui_frame(
    #[resource] pass: &mut UiPass,
    #[resource] ui_context: &mut UiContext,
    #[resource] device: &Device,
    #[resource] queue: &Queue,
    #[resource] current_frame: &SwapChainTexture,
    #[resource] ui_textures: &Assets<UiTexture>,
    #[resource] window_size: &WindowSize,
) {
    let (_output, commands) = ui_context.context.end_frame();
    let context = &ui_context.context;
    let paint_jobs = context.tessellate(commands);
    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("Ui command encoder"),
    });
    pass.update_texture(&device, &queue, &context.texture());
    pass.update_buffers(&device, &queue, &paint_jobs, &window_size);
    // Record all render passes.
    pass.draw(
        &mut encoder,
        &current_frame.view,
        &paint_jobs,
        ui_textures,
        &window_size,
    );
    pass.command_sender
        .send(encoder.finish())
        .expect("Failed to send ui_render commands");
}
