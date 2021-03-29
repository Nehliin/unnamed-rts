use egui::{paint::ClippedShape, vec2, CtxRef, RawInput};
use unnamed_rts::resources::WindowSize;
use winit::event::ModifiersState;

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
