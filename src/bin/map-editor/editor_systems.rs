use std::{path::Path, time::Instant};

use egui::CollapsingHeader;
use glam::{Vec2, Vec3A};
use legion::*;
use unnamed_rts::{
    assets::Handle,
    input::{CursorPosition, MouseButtonState},
    rendering::{
        camera::Camera,
        ui::ui_resources::{UiContext, UiTexture},
    },
    resources::{Time, WindowSize},
    tilemap::{DrawableTileMap, TileMap},
};
use winit::event::MouseButton;
#[derive(Debug, Default)]
pub struct EditorSettings {
    pub edit_tilemap: bool,
    pub tm_settings: TileEditorSettings,
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum TileEditMode {
    DisplacementMap,
    // TODO Take in texture
    ColorTexture,
}

// Settings for the heightmap
#[derive(Debug)]
pub struct TileEditorSettings {
    pub tool_strenght: u8,
    pub tool_size: f32,
    pub max_height: u8,
    pub mode: TileEditMode,
    pub draw_tile_types: bool,
    pub save_path: Option<String>,
    pub load_path: String,
}

impl Default for TileEditorSettings {
    fn default() -> Self {
        TileEditorSettings {
            tool_strenght: 1,
            tool_size: 20.0,
            max_height: 255,
            mode: TileEditMode::DisplacementMap,
            draw_tile_types: false,
            save_path: None,
            load_path: "my_map_name.map".to_string(),
        }
    }
}

pub struct UiState<'a> {
    pub img: Handle<UiTexture<'a>>,
    pub show_load_popup: bool,
    pub debug_tile_draw_on: bool,
    pub load_error_label: Option<String>,
}

#[system]
#[allow(clippy::too_many_arguments)]
// This system is a bit of spagettios but I will clean it up later. Features more important atm!
pub fn editor_ui(
    #[state] state: &mut UiState<'static>,
    #[resource] ui_context: &UiContext,
    #[resource] editor_settings: &mut EditorSettings,
    #[resource] tilemap: &mut DrawableTileMap<'static>,
    #[resource] window_size: &WindowSize,
    #[resource] device: &wgpu::Device,
    #[resource] queue: &wgpu::Queue,
) {
    if editor_settings.tm_settings.save_path.is_none() {
        editor_settings.tm_settings.save_path = Some(format!("assets/{}.map", tilemap.name()));
    }
    egui::SidePanel::left("editor_side_panel", 120.0).show(&ui_context.context, |ui| {
        ui.vertical_centered(|ui| {
            ui.checkbox(&mut editor_settings.edit_tilemap, "Edit Tilemap");
            if editor_settings.edit_tilemap {
                let settings = &mut editor_settings.tm_settings;
                CollapsingHeader::new("Tilemap settings")
                    .default_open(true)
                    .show(ui, |ui| {
                        egui::ComboBox::from_label("Edit mode")
                            .selected_text(format!("{:?}", settings.mode))
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut settings.mode,
                                    TileEditMode::DisplacementMap,
                                    "Displacement Map",
                                );
                                ui.selectable_value(
                                    &mut settings.mode,
                                    TileEditMode::ColorTexture,
                                    "Color Texture",
                                );
                            });
                        ui.add(
                            egui::Slider::new(&mut settings.tool_strenght, 0..=10)
                                .text("Strenght"),
                        );
                        if ui
                            .button("Reset current buffer")
                            .on_hover_text(
                                "Resets the currently modifyable buffer either the displacement map or color texture for the map",
                            )
                            .clicked()
                        {
                            match settings.mode {
                                TileEditMode::DisplacementMap => {
                                   tilemap.reset_displacment();
                                }
                                TileEditMode::ColorTexture => {
                                   tilemap.reset_color_layer(); 
                                }
                            }
                        }
                        ui.separator();
                        ui.checkbox(&mut settings.draw_tile_types, "Debug Draw tile types");
                        if settings.draw_tile_types {
                            if !state.debug_tile_draw_on {
                                state.debug_tile_draw_on = true;
                                info!("Will draw debug tiles!"); 
                            }
                        } else if state.debug_tile_draw_on {
                           tilemap.reset_debug_layer();
                           state.debug_tile_draw_on = false;
                           info!("Stop drawing debug layer!");
                        }

                        let save_path = settings.save_path.as_ref().expect("Name should be used as default value");
                        ui.label(format!("Path saved to: {}", save_path));
                        if ui.button("Save map").clicked() {
                            use std::io::prelude::*;
                            let mut file = std::fs::File::create(save_path).unwrap();
                            file.write_all(&tilemap.serialize().unwrap()).unwrap();
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
                                    match TileMap::load(Path::new(&settings.load_path)) {
                                        Ok(loaded_map) => {
                                            *tilemap = DrawableTileMap::new(&device, &queue, loaded_map);
                                            *load_error_label = None;
                                        },
                                        Err(err) => {
                                            *load_error_label = Some(format!("Error: {}", err));
                                        }
                                    }
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
                    tilemap.name(),
                    tilemap.size(),
                ));
            })
        });
    });
}

// TODO: This should be done in a more general way instead
pub struct LastTileMapUpdate {
    pub last_update: Instant,
}
const MAX_UPDATE_FREQ: f32 = 1.0 / 60.0;

#[allow(clippy::too_many_arguments)]
#[system]
pub fn tilemap_modification(
    #[state] last_update: &mut LastTileMapUpdate,
    #[resource] camera: &Camera,
    #[resource] mouse_button_state: &MouseButtonState,
    #[resource] mouse_pos: &CursorPosition,
    #[resource] window_size: &WindowSize,
    #[resource] time: &Time,
    #[resource] tilemap: &mut DrawableTileMap,
    #[resource] editor_settings: &EditorSettings,
) {
    if !editor_settings.edit_tilemap {
        return;
    }
    let ray = camera.raycast(mouse_pos, window_size);
    // check intersection with the heightmap
    let normal = Vec3A::new(0.0, 1.0, 0.0);
    let denominator = normal.dot(ray.direction);
    let tm_settings = &editor_settings.tm_settings;
    if denominator.abs() > 0.0001 {
        // it isn't parallel to the plane
        // (camera can still theoretically be within the height_map but don't care about that)
        let height_map_pos: Vec3A = tilemap.transform().translation.into();
        let t = (height_map_pos - ray.origin).dot(normal) / denominator;
        if t >= 0.0 {
            // there was an intersection
            let target = (t * ray.direction) + ray.origin;
            let tile_coords = tilemap.to_tile_coords(target);
            if tile_coords.is_none() {
                return;
            }
            let tile_coords = tile_coords.unwrap();
            if (time.current_time - last_update.last_update).as_secs_f32() <= MAX_UPDATE_FREQ {
                return;
            }
            last_update.last_update = time.current_time;
            if mouse_button_state.is_pressed(&MouseButton::Left) {
                match tm_settings.mode {
                    TileEditMode::DisplacementMap => {
                        tilemap.set_tile_height(
                            tile_coords.x,
                            tile_coords.y,
                            tm_settings.tool_strenght,
                        );
                    }
                    TileEditMode::ColorTexture => {
                        let radius = tm_settings.tool_size;
                        let center = tile_coords * tilemap.tile_texture_resolution();
                        let center = center.as_f32();
                        tilemap.modify_color_texels(|x, y, bytes| {
                            let distance = Vec2::new(x as f32, y as f32).distance(center);
                            if distance < radius {
                                bytes[0] = 255;
                                bytes[1] = 0;
                                bytes[2] = 0;
                                bytes[3] = 255;
                            }
                        });
                    }
                }
            }
            tilemap.reset_decal_layer();
            match tm_settings.mode {
                TileEditMode::DisplacementMap => {
                    tilemap.modify_tile_decal_texels(
                        tile_coords.x,
                        tile_coords.y,
                        |_, _, bytes| {
                            bytes[0] = 0;
                            bytes[1] = 255;
                            bytes[2] = 0;
                            bytes[3] = 255;
                        },
                    );
                }
                TileEditMode::ColorTexture => {
                    let radius = tm_settings.tool_size;
                    let center = tile_coords * tilemap.tile_texture_resolution();
                    let center = center.as_f32();
                    tilemap.modify_decal_texels(|x, y, bytes| {
                        let distance = Vec2::new(x as f32, y as f32).distance(center);
                        if (radius - 2.0) < distance && distance < radius {
                            bytes[0] = 0;
                            bytes[1] = 255;
                            bytes[2] = 0;
                            bytes[3] = 255;
                        }
                    });
                }
            }
        }
    }
}
