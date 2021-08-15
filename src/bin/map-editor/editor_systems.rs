use egui::CollapsingHeader;
use glam::{Affine3A, Quat, UVec2, Vec2, Vec3, Vec3A, Vec3Swizzles};
use itertools::Itertools;
use legion::{systems::CommandBuffer, world::SubWorld, *};
use std::{path::Path, time::Instant};
use unnamed_rts::{
    assets::{Assets, Handle},
    components::{Selectable, Transform},
    input::{CursorPosition, MouseButtonState},
    map_chunk::{ChunkIndex, CHUNK_SIZE},
    navigation::FlowField,
    rendering::{
        camera::Camera,
        drawable_tilemap::*,
        gltf::GltfModel,
        ui::ui_resources::{UiContext, UiTexture},
    },
    resources::{Time, WindowSize},
    tilemap::{TileMap, TILE_HEIGHT, TILE_WIDTH},
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
    pub current_tile: UVec2,
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
            current_tile: UVec2::new(0, 0),
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
// TODO: This system is a bit of spagettios but I will clean it up later. Features more important atm!
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
    egui::SidePanel::left("editor_side_panel")
        .resizable(false)
        .max_width(120.0)
        .show(&ui_context.context, |ui| {
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
                        let (tile_x, tile_y) = settings.current_tile.into();
                        if let Some(tile_type) = tilemap.tile(tile_x as i32,tile_y as i32).map(|tile| tile.tile_type) {
                            ui.label(format!("Current tile_type: {:?}", tile_type));
                        }
                        ui.checkbox(&mut settings.draw_tile_types, "Debug Draw tile types");
                        if settings.draw_tile_types {
                            if !state.debug_tile_draw_on {
                                state.debug_tile_draw_on = true;
                                info!("Will draw debug tiles!"); 
                                tilemap.fill_debug_layer();
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
                                            *tilemap = DrawableTileMap::new(device, queue, loaded_map);
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
    egui::TopBottomPanel::top("editor_top_panel").show(&ui_context.context, |ui| {
        ui.horizontal(|ui| {
            ui.columns(3, |columns| {
                columns[1].label(format!(
                    "Map editor: {}, chunk size: {}",
                    tilemap.name(),
                    CHUNK_SIZE,
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
    #[resource] editor_settings: &mut EditorSettings,
) {
    if !editor_settings.edit_tilemap {
        return;
    }
    let ray = camera.raycast(mouse_pos, window_size);
    // check intersection with the heightmap
    let normal = Vec3A::new(0.0, 1.0, 0.0);
    let denominator = normal.dot(ray.direction);
    let tm_settings = &mut editor_settings.tm_settings;
    if denominator.abs() > 0.0001 {
        // it isn't parallel to the plane
        // (camera can still theoretically be within the height_map but don't care about that)
        let height_map_pos: Vec3A = tilemap.tile_grid().transform().matrix.translation;
        let t = (height_map_pos - ray.origin).dot(normal) / denominator;
        if t >= 0.0 {
            // there was an intersection
            let target = (t * ray.direction) + ray.origin;
            let tile_coords = tilemap.to_tile_coords(target);
            if tile_coords.is_none() {
                return;
            }
            let tile_coords = tile_coords.unwrap();
            *tm_settings.current_tile = *tile_coords;
            if (time.current_time - last_update.last_update).as_secs_f32() <= MAX_UPDATE_FREQ {
                return;
            }
            last_update.last_update = time.current_time;
            if mouse_button_state.is_pressed(&MouseButton::Left) {
                match tm_settings.mode {
                    TileEditMode::DisplacementMap => {
                        tilemap.set_tile_height(
                            tile_coords.x as i32,
                            tile_coords.y as i32,
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
// TODO Move everything below to common systems --------------------------------

fn intesercts(origin: Vec3A, dirfrac: Vec3A, aabb_min: Vec3A, aabb_max: Vec3A) -> bool {
    let t1 = (aabb_min - origin) * dirfrac;
    let t2 = (aabb_max - origin) * dirfrac;

    let tmin = t1.min(t2);
    let tmin = tmin.max_element();

    let tmax = t1.max(t2);
    let tmax = tmax.min_element();

    !(tmax < 0.0 || tmax < tmin)
}

#[system]
pub fn selection(
    world: &mut SubWorld,
    #[resource] camera: &Camera,
    #[resource] mouse_button_state: &MouseButtonState,
    #[resource] mouse_pos: &CursorPosition,
    #[resource] asset_storage: &Assets<GltfModel>,
    #[resource] window_size: &WindowSize,
    query: &mut Query<(&Transform, &Handle<GltfModel>, &mut Selectable)>,
) {
    if mouse_button_state.pressed_current_frame(&MouseButton::Left) {
        let ray = camera.raycast(mouse_pos, window_size);
        let dirfrac = ray.direction.recip();
        query.par_for_each_mut(world, |(transform, handle, mut selectable)| {
            let model = asset_storage.get(handle).unwrap();
            let (min, max) = (model.min_vertex, model.max_vertex);
            let world_min = transform.matrix.transform_point3a(min.into());
            let world_max = transform.matrix.transform_point3a(max.into());
            selectable.is_selected = intesercts(
                camera.get_position(),
                dirfrac,
                world_min.xyz(),
                world_max.xyz(),
            );
        })
    }
}
//TODO: Associate each selected entity with a group which in turn gets assigned a flowfield
#[system]
#[allow(clippy::too_many_arguments)]
pub fn move_action(
    world: &mut SubWorld,
    command_buffer: &mut CommandBuffer,
    #[resource] camera: &Camera,
    #[resource] mouse_button_state: &MouseButtonState,
    #[resource] mouse_pos: &CursorPosition,
    #[resource] window_size: &WindowSize,
    #[resource] tilemap: &DrawableTileMap,
    query: &mut Query<(Entity, &Selectable)>,
) {
    if mouse_button_state.pressed_current_frame(&MouseButton::Right) {
        query.for_each(world, |(entity, selectable)| {
            if selectable.is_selected {
                let ray = camera.raycast(mouse_pos, window_size);
                // check intersection with the regular ground plan
                let normal = Vec3A::Y;
                let denominator = normal.dot(ray.direction);
                if denominator.abs() > 0.0001 {
                    // it isn't parallel to the plane
                    // (camera can still theoretically be within the plane but don't care about that)
                    let t = -(normal.dot(ray.origin)) / denominator;
                    if t >= 0.0 {
                        // there was an intersection
                        let target = (t * ray.direction) + ray.origin;
                        info!("Move target: {}", target);
                        if let Ok(index) = ChunkIndex::new(target.x as i32, target.y as i32) {
                            command_buffer.add_component(
                                *entity,
                                FlowField::new(index, tilemap.tile_grid()),
                            );
                        }
                    }
                }
            }
        });
    }
}

#[derive(Debug)]
pub struct DebugFlow {
    pub current_target: Option<ChunkIndex>,
    pub arrow_handle: Handle<GltfModel>,
}

fn look_at(direction: Vec3A) -> Quat {
    let mut rotation_axis = Vec3A::Z.cross(direction).normalize_or_zero();
    if rotation_axis.length_squared() < 0.001 {
        rotation_axis = Vec3A::Y;
    }
    let dot = Vec3A::Z.dot(direction);
    let angle = dot.acos();
    Quat::from_axis_angle(rotation_axis.into(), angle)
}

#[system]
pub fn movement(
    world: &mut SubWorld,
    command_buffer: &mut CommandBuffer,
    #[resource] tilemap: &mut DrawableTileMap,
    #[resource] redraw_flow: &mut DebugFlow,
    query: &mut Query<(Entity, &FlowField)>,
) {
    query.for_each(world, |(_entity, flow_field)| {
        if redraw_flow.current_target != Some(flow_field.target) {
            redraw_flow.current_target = Some(flow_field.target);
            tilemap.reset_debug_layer();
            let transform = *tilemap.tile_grid().transform();
            let debug_arrows = (0..CHUNK_SIZE)
                .cartesian_product(0..CHUNK_SIZE)
                .into_iter()
                .map(|(y, x)| {
                    let flow_tile = flow_field.grid.tile(ChunkIndex::new(x, y).unwrap());
                    tilemap.modify_tile_debug_texels(x as u32, y as u32, |_, _, buffer| {
                        buffer[1] = std::cmp::max(255_i32 - flow_tile.distance as i32, 41) as u8;
                        buffer[2] = std::cmp::max(127_i32 - flow_tile.distance as i32, 56) as u8;
                        buffer[3] = 255;
                        buffer[0] = 255 - buffer[1];
                    });
                    let height = tilemap
                        .tile_grid()
                        .tile(ChunkIndex::new(x, y).unwrap())
                        .middle_height();
                    // 1. calc offset for arrow
                    let translation = Vec3::new(
                        x as f32 * TILE_WIDTH + 0.5,
                        height + 0.7,
                        y as f32 * TILE_HEIGHT + 0.5,
                    );
                    let scale = Vec3::splat(0.1);
                    // 2. create Transform for it by providing pos, rotation, scale
                    let arrow_transform = Affine3A::from_scale_rotation_translation(
                        scale,
                        look_at(flow_tile.direction),
                        translation,
                    );
                    // 3. multiply transforms
                    (
                        redraw_flow.arrow_handle,
                        Transform {
                            matrix: transform.matrix * arrow_transform,
                        },
                    )
                })
                .collect::<Vec<_>>();
            // TODO: SPAWN THE ARROWS SOMEHOW AND THEN ALSO RENDER THEM WHEN NECESSARY
            command_buffer.extend(debug_arrows);
        }
    });
}
