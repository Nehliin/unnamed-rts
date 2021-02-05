use std::time::Instant;

use egui::{pos2, vec2, Color32, Modifiers};
use egui_demo_lib::DemoWindows;
use input::{CursorPosition, MouseMotion, Text};
use legion::*;
use wgpu::{CommandEncoderDescriptor, Device, Queue, SwapChainTexture};
use winit::event::{ModifiersState, MouseButton, MouseScrollDelta};

use crate::{
    application::Time,
    input::{self, FrameEvent},
};

use super::{
    ui_context::{self, UiContext, WindowSize},
    ui_pass::UiPass,
};

#[system]
pub fn update_ui(
    #[resource] ui_ctx: &mut UiContext,
    #[resource] window_size: &WindowSize,
    #[resource] modifiers_changed: &FrameEvent<ModifiersState>,
    #[resource] mouse_position: &CursorPosition,
    #[resource] mouse_scroll: &FrameEvent<MouseScrollDelta>,
    #[resource] text_input: &FrameEvent<Text>,
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
    // checkcursor moved after cursor left

    ui_ctx.raw_input.mouse_pos = Some(pos2(
        mouse_position.x as f32 / ui_ctx.raw_input.pixels_per_point.unwrap(),
        mouse_position.y as f32 / ui_ctx.raw_input.pixels_per_point.unwrap(),
    ));
    // println!("{:?}", mouse_input);
    ui_ctx.raw_input.mouse_down = mouse_input.pressed.contains(&MouseButton::Left);

    if let Some(scroll_delta) = mouse_scroll.event {
        match scroll_delta {
            MouseScrollDelta::LineDelta(x, y) => {
                let line_height = 24.0; // TODO as in egui_glium
                ui_ctx.raw_input.scroll_delta = vec2(x, y) * line_height;
            }
            MouseScrollDelta::PixelDelta(delta) => {
                // Actually point delta
                ui_ctx.raw_input.scroll_delta = vec2(delta.x as f32, delta.y as f32);
            }
        }
    }

    if let Some(text) = &text_input.event {
        println!("TEXT: {:?}", text);
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

    let modifiers = if let Some(modifier_state) = modifiers_changed.event {
        input::winit_to_egui_modifiers(modifier_state)
    } else {
        ui_ctx.raw_input.modifiers
    };

    for key in key_input.all_pressed_current_frame() {
        println!("virt Key: {:?}", key);
        if let Some(key) = input::winit_to_egui_key_code(key) {
            println!("Key: {:?}", key);
            ui_ctx.raw_input.events.push(egui::Event::Key {
                key,
                pressed: true,
                modifiers,
            })
        }
    }

    for key in key_input.all_release_current_frame() {
        if let Some(key) = input::winit_to_egui_key_code(key) {
            ui_ctx.raw_input.events.push(egui::Event::Key {
                key,
                pressed: false,
                modifiers,
            })
        }
    }
}

#[system]
pub fn test(#[state] demo: &mut DemoWindows, #[resource] ui: &UiContext) {
    demo.ui(&ui.context)
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
pub fn begin_ui_frame(#[state] time_since_start: &Instant, #[resource] ui_context: &mut UiContext) {
    ui_context.update_time(time_since_start.elapsed().as_secs_f64());
    ui_context.begin_frame();
}

#[system]
pub fn draw_fps_counter(#[resource] ui_context: &UiContext, #[resource] time: &Time) {
    egui::Area::new("FPS area")
        .fixed_pos(egui::pos2(0.0, 0.0))
        .show(&ui_context.context, |ui| {
            let label = egui::Label::new(format!("FPS: {}", 1.0 / time.delta_time))
                .text_color(Color32::WHITE);
            ui.add(label);
        });
}

// TODO: handle user textures here
// Basicall simply load texture data to create a egui::Texture and then run egui texture to wgpu texture
// However, better to map texture id = already loaded texture (via Aseets<>) and handle it from there
#[system]
pub fn end_ui_frame(
    #[state] pass: &mut UiPass,
    #[resource] ui_context: &mut UiContext,
    #[resource] device: &Device,
    #[resource] queue: &Queue,
    #[resource] current_frame: &SwapChainTexture,
    #[resource] window_size: &WindowSize,
) {
    let (_output, commands) = ui_context.end_frame();
    let context = &ui_context.context;
    let paint_jobs = context.tessellate(commands);
    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("Ui command encoder"),
    });
    pass.update_texture(&device, &queue, &context.texture());
    pass.update_user_textures(&device, &queue);
    pass.update_buffers(&device, &queue, &paint_jobs, &window_size);
    // Record all render passes.
    pass.execute(
        &mut encoder,
        &current_frame.view,
        &paint_jobs,
        &window_size,
    );
    pass.command_sender
        .send(encoder.finish())
        .expect("Failed to send ui_render commands");
}
