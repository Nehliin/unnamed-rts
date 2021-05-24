use std::{ops::Index, time::Instant};

use egui::CollapsingHeader;
use glam::{vec2, UVec2, Vec2, Vec3A, Vec4, Vec4Swizzles};
use legion::*;
use rayon::prelude::*;
use unnamed_rts::{
    assets::{Assets, Handle},
    input::{CursorPosition, MouseButtonState},
    rendering::{
        camera::Camera,
        heightmap_pass::HeightMap,
        texture::TextureContent,
        ui::ui_resources::{UiContext, UiTexture},
    },
    resources::{Time, WindowSize},
    tilemap::{DrawableTileMap, Tile, TILE_HEIGHT, TILE_WIDTH},
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

// TODO: The actual buffer modification part of this should
// probably live in a HeightMapTool trait
#[derive(Debug, PartialEq, Clone, Copy)]
pub enum HmEditorTool {
    Square,
    Circle,
}

// Settings for the heightmap
#[derive(Debug)]
pub struct HmEditorSettings {
    pub tool: HmEditorTool,
    pub tool_strenght: u8,
    pub tool_size: f32,
    pub max_height: u8,
    pub inverted: bool,
    pub mode: HmEditorMode,
    pub save_path: Option<String>,
    pub load_path: String,
}

impl Default for HmEditorSettings {
    fn default() -> Self {
        HmEditorSettings {
            tool: HmEditorTool::Circle,
            tool_strenght: 1,
            tool_size: 20.0,
            max_height: 255,
            inverted: false,
            mode: HmEditorMode::DisplacementMap,
            save_path: None,
            load_path: "my_map_name.map".to_string(),
        }
    }
}

pub struct UiState<'a> {
    pub img: Handle<UiTexture<'a>>,
    pub show_load_popup: bool,
    pub load_error_label: Option<String>,
}

#[system]
#[allow(clippy::too_many_arguments)]
pub fn editor_ui(
    #[state] state: &mut UiState<'static>,
    #[resource] ui_context: &UiContext,
    #[resource] editor_settings: &mut EditorSettings,
    #[resource] tilemap: &mut DrawableTileMap<'static>,
    #[resource] window_size: &WindowSize,
    //#[resource] hm_assets: &mut Assets<HeightMap<'static>>,
    //#[resource] device: &wgpu::Device,
    //#[resource] queue: &wgpu::Queue,
) {
    if editor_settings.hm_settings.save_path.is_none() {
        editor_settings.hm_settings.save_path = Some(format!("assets/{}.map", tilemap.map.name));
    }
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
                            egui::Slider::new(&mut settings.tool_strenght, 1..=10)
                                .text("Strenght"),
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
                                    tilemap.map.tiles.fill(Tile::default());
                                }
                                HmEditorMode::ColorTexture => {
                                    /*let map_size = tilemap.map.size;
                                    let (_, buffer) = tilemap.get_color_buffer_mut();
                                   // TODO: unecessary allocation here but not as important within the editor
                                    let checkerd = TextureContent::checkerd(map_size);
                                    buffer.copy_from_slice(&checkerd.bytes);*/
                                    todo!("Fix  textures");
                                }
                            }
                        }
                        let save_path = settings.save_path.as_ref().expect("Name should be used as default value");
                        ui.label(format!("Path saved to: {}", save_path));
                        if ui.button("Save map").clicked() {
                            use std::io::prelude::*;
                            let seializable = &tilemap.map;
                            let mut file = std::fs::File::create(save_path).unwrap();
                            file.write_all(&bincode::serialize(seializable).unwrap()).unwrap();
                        }
                        if ui.button("Load map").clicked() {
                            state.show_load_popup = true;
                        }
                        let (x, y) = window_size.logical_size();
                        let show_load_popup = &mut state.show_load_popup;
                        let load_error_label = &mut state.load_error_label;
                        egui::Window::new("Load map")
                            .open(show_load_popup)
                            .resizable(false)
                            .collapsible(false)
                            .fixed_pos((x as f32/2.0, y as f32 /2.0 ))
                            .show(&ui_context.context, |ui| {
                                ui.text_edit_singleline(&mut settings.load_path);
                                if ui.button("Load").clicked() {
                                    /*match hm_assets.load_immediate(&settings.load_path, device, queue) {
                                        Ok(loaded_map) => {
                                            *tilemap = loaded_map;
                                            *load_error_label = None;
                                        },
                                        Err(err) => {
                                            *load_error_label = Some(format!("Error: {}", err));
                                        }
                                    }*/
                                    todo!("Fix loading!");
                                }
                                if let Some(load_error_label) = load_error_label.as_ref() {
                                    ui.add(egui::Label::new(load_error_label).text_color(egui::Color32::RED));
                                }
                            });
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
                    "Map editor: {}, size: {}",
                    tilemap.map.name, tilemap.map.size,
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
pub fn tilemap_modification(
    #[state] modification_state: &mut HeightMapModificationState,
    #[resource] camera: &Camera,
    #[resource] mouse_button_state: &MouseButtonState,
    #[resource] mouse_pos: &CursorPosition,
    #[resource] window_size: &WindowSize,
    #[resource] time: &Time,
    #[resource] tilemap: &mut DrawableTileMap,
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
        let height_map_pos: Vec3A = tilemap.map.transform.translation.into();
        let t = (height_map_pos - ray.origin).dot(normal) / denominator;
        if t >= 0.0 {
            // there was an intersection
            let target = (t * ray.direction) + ray.origin;
            let tile_coords = tilemap.map.to_tile_coords(target);
            if tile_coords.is_none() {
                return;
            }
            let tile_coords = tile_coords.unwrap();
            if (time.current_time - modification_state.last_update).as_secs_f32() <= MAX_UPDATE_FREQ
            {
                return;
            }
            modification_state.last_update = time.current_time;

            if mouse_button_state.is_pressed(&MouseButton::Left) {
                match hm_settings.tool {
                    HmEditorTool::Square => {
                        // update_height_map_square(tilemap, hm_settings, center);
                    }
                    HmEditorTool::Circle => {
                        //update_height_map_circular(tilemap, hm_settings, center);
                    }
                }
            }
            let map_size = tilemap.map.size;
            // TODO: only do this if the intersection is within bounds
            let (stride, buffer) = tilemap.render_data.decal_buffer_mut();
            // clear previous decal value, this is innefficient and should be changed to only clear previous
            // marked radius to avoid removing unrelated things in the decal layer
            buffer.fill(0);
            match hm_settings.tool {
                HmEditorTool::Square => {
                    draw_square_decal(stride, tile_coords, buffer, hm_settings, map_size);
                }
                HmEditorTool::Circle => {
                    //draw_circle_decal(stride, center, buffer, hm_settings, map_size);
                }
            }
        }
    }
}

fn draw_circle_decal(
    stride: u32,
    center: Vec2,
    buffer: &mut [u8],
    hm_settings: &HmEditorSettings,
    map_size: u32,
) {
    let radius = hm_settings.tool_size;
    buffer
        .par_chunks_exact_mut((map_size * stride) as usize)
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

fn draw_square_decal(
    stride: u32,
    tile_coords: UVec2,
    buffer: &mut [u8],
    _hm_settings: &HmEditorSettings,
    map_size: u32,
) {
    // First off the pixel buffer is flipped with respect to the y axis
    // The decal layer is currently running one TILE_X pixels per tile which works for selection
    // but not much else
    buffer[0] = 255;
    buffer[5] = 255;
    buffer[10] = 255;
    let row = (stride * TILE_WIDTH as u32 * map_size) as usize;
    buffer[row - 2] = 255;
    buffer[row * ( map_size - 1) as usize - 2] = 255;
    //TODO: Handle scaling of tiles here
    let to_buffer_coords = |coord: UVec2| {
        let row_size = TILE_WIDTH as u32 * stride * map_size;
        let mut row = coord.y * row_size;
        // Wrong constant here?
        row += coord.x * TILE_WIDTH as u32 * stride;
        row as usize
    };
    /* for x in tile_coords.x..tile_coords.x + (TILE_WIDTH as u32 * stride) {
        for y in tile_coords.y..tile_coords.y + (TILE_HEIGHT  as u32 * stride) {
            // SWAP ME
            let index = to_buffer_coords(UVec2::new(y, x));
            buffer[index] = 0;
            buffer[index + 1] = 255;
            buffer[index + 2] = 0;
            buffer[index + 3] = 255;
        }
    } */
    /* let size_vec = Vec2::splat(size);
    let scaled_size_vec = Vec2::splat(size + 2.0);

    buffer
        .par_chunks_exact_mut((map_size * stride) as usize)
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
                    if within_outer.all() && !within_inner.all() {
                        bytes[0] = 0;
                        bytes[1] = 255;
                        bytes[2] = 0;
                        bytes[3] = 255;
                    }
                })
        }); */
}

fn update_height_map_circular(
    height_map: &mut HeightMap,
    hm_settings: &HmEditorSettings,
    center: Vec2,
) {
    let radius = hm_settings.tool_size;
    let strength = hm_settings.tool_strenght as f32;
    // assuming row order
    // TODO: Not very performance frendly
    match hm_settings.mode {
        HmEditorMode::DisplacementMap => {
            let map_size = height_map.get_size();
            let (_, buffer) = height_map.get_displacement_buffer_mut();
            buffer
                .par_chunks_exact_mut(map_size as usize)
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
            let map_size = height_map.get_size();
            let (stride, buffer) = height_map.get_color_buffer_mut();
            buffer
                .par_chunks_exact_mut((map_size * stride) as usize)
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
    let strength = hm_settings.tool_strenght as f32;

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
            let map_size = height_map.get_size();
            let (_, buffer) = height_map.get_displacement_buffer_mut();
            buffer
                .par_chunks_exact_mut(map_size as usize)
                .enumerate()
                .for_each(|(y, chunk)| {
                    chunk.par_iter_mut().enumerate().for_each(|(x, byte)| {
                        let pos = vec2(x as f32, y as f32);
                        let within_inner =
                            pos.cmpge(center - size_vec) & pos.cmple(center + size_vec);
                        let within_outer = pos.cmpge(center - scaled_size_vec)
                            & pos.cmple(center + scaled_size_vec);
                        if within_inner.all() {
                            proportional_change(byte, 1.0);
                        } else if within_outer.all() {
                            // bitmask is used to check individual boolean values of the vec
                            if within_inner.bitmask() & 1 != 0 {
                                // // area
                                let sign = (pos.y - center.y).signum();
                                let d = pos.distance(Vec2::new(pos.x, center.y + size * sign));
                                let max = scaled_size_vec.x;
                                proportional_change(byte, (max - d) / max);
                            } else if within_inner.bitmask() & (1 << 1) != 0 {
                                // xx area
                                let sign = (pos.x - center.x).signum();
                                let d = pos.distance(Vec2::new(center.x + size * sign, pos.y));
                                proportional_change(byte, (scaled_size - d) / scaled_size);
                            } else {
                                // oo area
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
