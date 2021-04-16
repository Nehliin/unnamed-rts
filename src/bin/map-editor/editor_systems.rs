use std::time::Instant;

use egui::CollapsingHeader;
use glam::{vec2, Vec2, Vec3A, Vec4, Vec4Swizzles};
use legion::*;
use rayon::prelude::*;
use unnamed_rts::{
    assets::Handle,
    graphics::{camera::Camera, heightmap_pass::HeightMap, texture::TextureContent},
    input::{CursorPosition, MouseButtonState},
    resources::{Time, WindowSize},
    ui::ui_resources::{UiContext, UiTexture},
};
use winit::event::MouseButton;
#[derive(Debug, Default)]
pub struct EditorSettings {
    pub edit_heightmap: bool,
    pub hm_settings: HmEditorSettings,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum HmEditorMode {
    DisplacementMap,
    // TODO Take in texture
    ColorTexture,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum HmEditorTool {
    Square,
    Circle,
}

// Settings for the heightmap
#[derive(Debug)]
pub struct HmEditorSettings {
    pub tool: HmEditorTool,
    pub tool_strenght: f32,
    pub tool_size: f32,
    pub max_height: u8,
    pub inverted: bool,
    pub map_size: u32,
    pub mode: HmEditorMode,
}

impl Default for HmEditorSettings {
    fn default() -> Self {
        HmEditorSettings {
            tool: HmEditorTool::Circle,
            tool_strenght: 5.0,
            tool_size: 20.0,
            max_height: 255,
            inverted: false,
            mode: HmEditorMode::DisplacementMap,
            map_size: 256,
        }
    }
}

pub struct Images<'a> {
    pub img: Handle<UiTexture<'a>>,
}

#[system]
pub fn editor_ui(
    #[state] state: &mut Images<'static>,
    #[resource] ui_context: &UiContext,
    #[resource] editor_settings: &mut EditorSettings,
    #[resource] height_map: &mut HeightMap, // maybe move this
) {
    egui::SidePanel::left("editor_side_panel", 120.0).show(&ui_context.context, |ui| {
        ui.vertical_centered(|ui| {
            ui.checkbox(&mut editor_settings.edit_heightmap, "Edit heightmap");
            if editor_settings.edit_heightmap {
                let settings = &mut editor_settings.hm_settings;
                CollapsingHeader::new("Heightmap settings")
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.columns(2, |uis| {
                            uis[0].selectable_value(
                                &mut settings.tool,
                                HmEditorTool::Circle,
                                "Circle",
                            );
                            uis[1].selectable_value(
                                &mut settings.tool,
                                HmEditorTool::Square,
                                "Square",
                            );
                        });
                        egui::ComboBox::from_label("Edit mode")
                            .selected_text(format!("{:?}", settings.mode))
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut settings.mode,
                                    HmEditorMode::DisplacementMap,
                                    "Displacement Map",
                                );
                                ui.selectable_value(
                                    &mut settings.mode,
                                    HmEditorMode::ColorTexture,
                                    "Color Texture",
                                );
                            });
                        ui.add(
                            egui::Slider::new(&mut settings.tool_size, 1.0..=300.0).text("Size"),
                        );
                        ui.add(
                            egui::Slider::new(&mut settings.tool_strenght, 1.0..=10.0)
                                .text("Strenght"),
                        );
                        ui.add(
                            egui::Slider::new(&mut settings.max_height, 0..=255).text("Max height"),
                        );
                        ui.checkbox(&mut settings.inverted, "Invert");
                        if ui
                            .button("Reset current buffer")
                            .on_hover_text(
                                "Resets the currently modifyable buffer either the displacement map or color texture for the map",
                            )
                            .clicked()
                        {
                            match settings.mode {
                                HmEditorMode::DisplacementMap => {
                                    let (_, buffer) = height_map.get_displacement_buffer_mut();
                                    buffer.fill(0);
                                }
                                HmEditorMode::ColorTexture => {
                                    let (_, buffer) = height_map.get_color_buffer_mut();
                                   // TODO: unecessary allocation here but not as important within the editor
                                    let checkerd = TextureContent::checkerd(settings.map_size);
                                    buffer.copy_from_slice(&checkerd.bytes);
                                }
                            }
                        }
                        // test user textures
                        let handle = state.img.into();
                        ui.image(handle, [50.0, 50.0]);
                    });
            }
        });
    });
    egui::TopPanel::top("editor_top_panel").show(&ui_context.context, |ui| {
        ui.horizontal(|ui| {
            ui.columns(3, |columns| {
                columns[1].label(format!(
                    "Map editor: <name>, size: {}",
                    editor_settings.hm_settings.map_size
                ));
            })
        });
    });
}

// TODO: This should be done in a more general way instead
pub struct HeightMapModificationState {
    pub last_update: Instant,
}
const MAX_UPDATE_FREQ: f32 = 1.0 / 60.0;

#[allow(clippy::too_many_arguments)]
#[system]
pub fn height_map_modification(
    #[state] modification_state: &mut HeightMapModificationState,
    #[resource] camera: &Camera,
    #[resource] mouse_button_state: &MouseButtonState,
    #[resource] mouse_pos: &CursorPosition,
    #[resource] window_size: &WindowSize,
    #[resource] time: &Time,
    #[resource] height_map: &mut HeightMap,
    #[resource] editor_settings: &EditorSettings,
) {
    if !editor_settings.edit_heightmap {
        return;
    }
    let ray = camera.raycast(mouse_pos, window_size);
    // check intersection with the heightmap
    let normal = Vec3A::new(0.0, 1.0, 0.0);
    let denominator = normal.dot(ray.direction);
    let hm_settings = &editor_settings.hm_settings;
    if denominator.abs() > 0.0001 {
        // it isn't parallel to the plane
        // (camera can still theoretically be within the height_map but don't care about that)
        let height_map_pos: Vec3A = height_map.get_transform().translation.into();
        let t = (height_map_pos - ray.origin).dot(normal) / denominator;
        if t >= 0.0 {
            // there was an intersection
            let target = (t * ray.direction) + ray.origin;
            // TODO: inbounds check here?
            let local_coords = height_map.get_transform().get_model_matrix().inverse()
                * Vec4::new(target.x, target.y, target.z, 1.0);
            let center = local_coords.xy();
            if (time.current_time - modification_state.last_update).as_secs_f32() <= MAX_UPDATE_FREQ
            {
                return;
            }
            modification_state.last_update = time.current_time;

            if mouse_button_state.is_pressed(&MouseButton::Left) {
                match hm_settings.tool {
                    HmEditorTool::Square => {
                        update_height_map_square(height_map, hm_settings, center);
                    }
                    HmEditorTool::Circle => {
                        update_height_map_circular(height_map, hm_settings, center);
                    }
                }
            }
            // TODO: only do this if the intersection is within bounds
            let (stride, buffer) = height_map.get_decal_buffer_mut();
            // clear previous decal value, this is innefficient and should be changed to only clear previous
            // marked radius to avoid removing unrelated things in the decal layer
            buffer.fill(0);
            match hm_settings.tool {
                HmEditorTool::Square => {
                    draw_square_decal(stride, center, buffer, hm_settings);
                }
                HmEditorTool::Circle => {
                    draw_circle_decal(stride, center, buffer, hm_settings);
                }
            }
        }
    }
}

fn draw_circle_decal(stride: u32, center: Vec2, buffer: &mut [u8], hm_settings: &HmEditorSettings) {
    let radius = hm_settings.tool_size;
    buffer
        .par_chunks_exact_mut((hm_settings.map_size * stride) as usize)
        .enumerate()
        .for_each(|(y, chunk)| {
            chunk
                .chunks_exact_mut(stride as usize)
                .enumerate()
                .for_each(|(x, bytes)| {
                    let distance = Vec2::new(x as f32, y as f32).distance(center);
                    if (radius - 2.0) < distance && distance < radius {
                        bytes[0] = 0;
                        bytes[1] = 255;
                        bytes[2] = 0;
                        bytes[3] = 255;
                    }
                })
        });
}

fn draw_square_decal(stride: u32, center: Vec2, buffer: &mut [u8], hm_settings: &HmEditorSettings) {
    let size = hm_settings.tool_size;
    let size_vec = Vec2::splat(size);
    let scaled_size_vec = Vec2::splat(size + 2.0);

    buffer
        .par_chunks_exact_mut((hm_settings.map_size * stride) as usize)
        .enumerate()
        .for_each(|(y, chunk)| {
            chunk
                .chunks_exact_mut(stride as usize)
                .enumerate()
                .for_each(|(x, bytes)| {
                    let pos = vec2(x as f32, y as f32);
                    let within_outer =
                        pos.cmpge(center - scaled_size_vec) & pos.cmple(center + scaled_size_vec);
                    let within_inner = pos.cmpge(center - size_vec) & pos.cmple(center + size_vec);
                    //let in_between = within_outer & !within_inner;
                    if within_outer.all() && !within_inner.all() {
                        bytes[0] = 0;
                        bytes[1] = 255;
                        bytes[2] = 0;
                        bytes[3] = 255;
                    }
                })
        });
}

fn update_height_map_circular(
    height_map: &mut HeightMap,
    hm_settings: &HmEditorSettings,
    center: Vec2,
) {
    let radius = hm_settings.tool_size;
    let strength = hm_settings.tool_strenght;
    // assuming row order
    // TODO: Not very performance frendly
    match hm_settings.mode {
        HmEditorMode::DisplacementMap => {
            let (_, buffer) = height_map.get_displacement_buffer_mut();
            buffer
                .par_chunks_exact_mut(hm_settings.map_size as usize)
                .enumerate()
                .for_each(|(y, chunk)| {
                    chunk.par_iter_mut().enumerate().for_each(|(x, byte)| {
                        let distance = Vec2::new(x as f32, y as f32).distance(center);
                        if distance < radius {
                            let raise = strength * (radius - distance) / radius;
                            if hm_settings.inverted {
                                *byte =
                                    std::cmp::max(0, (*byte as f32 - raise as f32).round() as u32)
                                        as u8;
                            } else {
                                *byte = std::cmp::min(
                                    hm_settings.max_height as u32,
                                    (*byte as f32 + raise as f32).round() as u32,
                                ) as u8;
                            };
                        }
                    })
                });
        }
        HmEditorMode::ColorTexture => {
            let (stride, buffer) = height_map.get_color_buffer_mut();
            buffer
                .par_chunks_exact_mut((hm_settings.map_size * stride) as usize)
                .enumerate()
                .for_each(|(y, chunk)| {
                    chunk
                        .chunks_exact_mut(stride as usize)
                        .enumerate()
                        .for_each(|(x, bytes)| {
                            let distance = Vec2::new(x as f32, y as f32).distance(center);
                            if distance < radius {
                                let color = strength * (radius - distance) / radius;
                                if hm_settings.inverted {
                                    let val = std::cmp::max(
                                        0,
                                        (bytes[0] as f32 - color as f32).round() as u32,
                                    ) as u8;
                                    // Shouldn't harde code indexes here..
                                    bytes[0] = val;
                                    bytes[1] = 0;
                                    bytes[2] = 0;
                                    bytes[3] = 0;
                                } else {
                                    let val = std::cmp::min(
                                        255,
                                        (bytes[0] as f32 + color as f32).round() as u32,
                                    ) as u8;
                                    bytes[0] = val;
                                    bytes[1] = 0;
                                    bytes[2] = 0;
                                    bytes[3] = 0;
                                };
                            }
                        })
                });
        }
    }
}

fn update_height_map_square(
    height_map: &mut HeightMap,
    hm_settings: &HmEditorSettings,
    center: Vec2,
) {
    let size = hm_settings.tool_size;
    let scale_factor = 1.9;
    let scaled_size = size * scale_factor;
    let size_vec = Vec2::splat(size);
    let scaled_size_vec = Vec2::splat(scaled_size);
    //let outer_size_factor = 2.5;
    let strength = hm_settings.tool_strenght;
    // extract out?
    let proportional_change = |byte: &mut u8, proportion: f32| {
        if hm_settings.inverted {
            *byte = std::cmp::max(0, (*byte as f32 - strength * proportion).round() as u32) as u8;
        } else {
            *byte = std::cmp::min(
                hm_settings.max_height as u32,
                (*byte as f32 + strength * proportion).round() as u32,
            ) as u8;
        };
    };
    let dist_center_to_corner = scaled_size - size;
    // assuming row order
    // TODO: Not very performance frendly
    match hm_settings.mode {
        HmEditorMode::DisplacementMap => {
            let (_, buffer) = height_map.get_displacement_buffer_mut();
            buffer
                .par_chunks_exact_mut(hm_settings.map_size as usize)
                .enumerate()
                .for_each(|(y, chunk)| {
                    chunk.par_iter_mut().enumerate().for_each(|(x, byte)| {
                        let pos = vec2(x as f32, y as f32);
                        let witin_inner =
                            pos.cmpge(center - size_vec) & pos.cmple(center + size_vec);
                        let witin_outer = pos.cmpge(center - scaled_size_vec)
                            & pos.cmple(center + scaled_size_vec);
                        if witin_inner.all() {
                            proportional_change(byte, 1.0);
                        } else if witin_outer.all() {
                            // bitmask is used to check individual boolean values of the vec
                            if witin_inner.bitmask() & 1 != 0 {
                                // // area
                                let sign = (pos.y - center.y).signum();
                                let d = pos.distance(Vec2::new(pos.x, center.y + size * sign));
                                let max = scaled_size_vec.x;
                                proportional_change(byte, (max - d) / max);
                            } else if witin_inner.bitmask() & (1 << 1) != 0 {
                                // xx area
                                let sign = (pos.x - center.x).signum();
                                let d = pos.distance(Vec2::new(center.x + size * sign, pos.y));
                                proportional_change(byte, (scaled_size - d) / scaled_size);
                            } else {
                                // oo area
                                // doesn't work?
                                let dist_to_center = pos.distance(center);
                                let d = dist_to_center - dist_center_to_corner;
                                // god knows why this relation to the scalefactor holds true but it works?
                                let boost = 2.0 - scale_factor;
                                proportional_change(byte, (scaled_size - d) / scaled_size + boost);
                            }
                        }
                    })
                });
        }
        HmEditorMode::ColorTexture => {
            todo!("Haven't implemented yet")
        }
    }
}
