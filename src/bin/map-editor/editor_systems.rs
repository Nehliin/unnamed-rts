use std::time::Instant;

use egui::CollapsingHeader;
use glam::{UVec2, Vec2, Vec3A};
use legion::*;
use rayon::prelude::*;
use unnamed_rts::{
    assets::Handle,
    input::{CursorPosition, MouseButtonState},
    rendering::{
        camera::Camera,
        ui::ui_resources::{UiContext, UiTexture},
    },
    resources::{Time, WindowSize},
    tilemap::{DrawableTileMap, TileMap, TileMapRenderData},
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

// Settings for the heightmap
#[derive(Debug)]
pub struct HmEditorSettings {
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
        editor_settings.hm_settings.save_path = Some(format!("assets/{}.map", tilemap.map.name()));
    }
    egui::SidePanel::left("editor_side_panel", 120.0).show(&ui_context.context, |ui| {
        ui.vertical_centered(|ui| {
            ui.checkbox(&mut editor_settings.edit_heightmap, "Edit heightmap");
            if editor_settings.edit_heightmap {
                let settings = &mut editor_settings.hm_settings;
                CollapsingHeader::new("Heightmap settings")
                    .default_open(true)
                    .show(ui, |ui| {
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
                            egui::Slider::new(&mut settings.tool_strenght, 0..=10)
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
                                   tilemap.map.reset();
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
                    tilemap.map.name(),
                    tilemap.map.size(),
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
        let height_map_pos: Vec3A = tilemap.map.transform().translation.into();
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
                update_height_map_square(tile_coords, &mut tilemap.map, hm_settings);
            }
            let map_size = tilemap.map.size();
            draw_square_decal(tile_coords, &mut tilemap.render_data, map_size);
        }
    }
}

fn draw_circle_decal(
    tile_coords: UVec2,
    render_data: &mut TileMapRenderData,
    hm_settings: &HmEditorSettings,
    map_size: u32,
) {
    let width_resolution = render_data.tile_width_resultion;
    let center = tile_coords.as_f32();
    let radius = hm_settings.tool_size;
    let (stride, buffer) = render_data.decal_buffer_mut();
    let row_size = stride * map_size * width_resolution;
    buffer.fill(0);
    buffer
        .par_chunks_exact_mut(row_size as usize)
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

fn draw_square_decal(tile_coords: UVec2, render_data: &mut TileMapRenderData, map_size: u32) {
    let width_resolution = render_data.tile_width_resultion;
    let height_resolution = render_data.tile_height_resultion;
    let (stride, buffer) = render_data.decal_buffer_mut();
    buffer.fill(0);
    let row_size = stride * map_size * width_resolution;
    let buffer_start = (tile_coords.y * row_size * height_resolution) as usize;
    let buffer_end = ((tile_coords.y + 1) * row_size * height_resolution) as usize;
    buffer[buffer_start..buffer_end]
        .par_chunks_exact_mut(row_size as usize)
        .for_each(|chunk| {
            chunk
                .chunks_exact_mut(stride as usize)
                .enumerate()
                .for_each(|(x, bytes)| {
                    let start = tile_coords.x * width_resolution;
                    let end = start + height_resolution;
                    if (start as usize) <= x && x < end as usize {
                        bytes[0] = 0;
                        bytes[1] = 255;
                        bytes[2] = 0;
                        bytes[3] = 255;
                    }
                });
        });
}

fn update_height_map_square(
    tile_coords: UVec2,
    tilemap: &mut TileMap,
    hm_settings: &HmEditorSettings,
) {
    match hm_settings.mode {
        HmEditorMode::DisplacementMap => {
            tilemap.set_tile_height(
                tile_coords.x,
                tile_coords.y,
                hm_settings.tool_strenght as f32,
                false,
            );
        }
        HmEditorMode::ColorTexture => {
            todo!("Haven't implemented yet")
        }
    }
}
