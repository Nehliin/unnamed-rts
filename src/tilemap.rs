use anyhow::Result;
use bytemuck::{Pod, Zeroable};
use glam::{UVec2, Vec2, Vec3, Vec3A};
use rayon::iter::IndexedParallelIterator;
use rayon::{
    iter::{IntoParallelIterator, ParallelIterator},
    slice::ParallelSliceMut,
};
use serde::{Deserialize, Serialize};

use crate::components::Transform;
#[repr(C)]
#[derive(Debug, Default, Pod, Zeroable, Deserialize, Serialize, Clone, Copy)]
pub struct TileVertex {
    position: Vec3,
    uv: Vec2,
}
#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub enum TileType {
    Flat,
    RampTop,
    RampBottom,
    RampRight,
    RampLeft,
    CornerConcaveRT,
    CornerConvexRT,
    CornerConcaveLT,
    CornerConvexLT,
    CornerConcaveRB,
    CornerConvexRB,
    CornerConcaveLB,
    CornerConvexLB,
}

impl Default for TileType {
    fn default() -> Self {
        TileType::Flat
    }
}

const VERTECIES_PER_TILE: usize = 9;
const INDICIES_PER_TILE: usize = 24;
const TILE_WIDTH: f32 = 2.0;
const TILE_HEIGHT: f32 = 2.0;
// Z coords per tile?

#[derive(Debug, Copy, Clone)]
/// Describes the Tile edge vertices
enum TileEdge {
    TopLeft = 0,
    TopMiddle = 1,
    TopRight = 2,
    MiddleLeft = 3,
    MiddleRight = 5,
    BottomLeft = 6,
    BottomMiddle = 7,
    BottomRight = 8,
}

#[derive(Debug)]
/// Describes a tile together with an edge vertex relative to the tile
/// Used to list adjacent tiles to a given corner
struct EdgeAdjacentTile {
    edge: TileEdge,
    tile_idx: Option<usize>,
}

/*
 *   *------*------*
 *   |\   1 | 3   /|
 *   |  \   |   /  |
 *   | 2  \ | /  4 |
 *   *------*------*
 *   | 5  / | \  7 |
 *   |  /   |   \  |
 *   |/  6  | 8   \|
 *   *------*------*
 */
#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct Tile {
    verticies: [TileVertex; VERTECIES_PER_TILE],
    indicies: [u32; INDICIES_PER_TILE],
    tile_type: TileType,
    base_height: f32,
    // ramp height
}

impl Tile {
    pub fn new(top_left_corner: Vec3, start_idx: u32, size: u32) -> Self {
        let height = size as f32 * TILE_HEIGHT;
        let width = size as f32 * TILE_WIDTH;
        // TODO change order of this to make indices closer to each other
        let verticies = [
            // Top left
            TileVertex {
                position: top_left_corner,
                uv: Vec2::new(top_left_corner.x / width, top_left_corner.z / height),
            },
            // Top middle
            TileVertex {
                position: top_left_corner + Vec3::X * TILE_WIDTH / 2.0,
                uv: Vec2::new(
                    (top_left_corner + Vec3::X * TILE_WIDTH / 2.0).x / width,
                    (top_left_corner + Vec3::X * TILE_WIDTH / 2.0).z / height,
                ),
            },
            // Top right
            TileVertex {
                position: top_left_corner + Vec3::X * TILE_WIDTH,
                uv: Vec2::new(
                    (top_left_corner + Vec3::X * TILE_WIDTH).x / width,
                    (top_left_corner + Vec3::X * TILE_WIDTH).z / height,
                ),
            },
            // Middle left
            TileVertex {
                position: top_left_corner + Vec3::Z * TILE_HEIGHT / 2.0,
                uv: Vec2::new(
                    (top_left_corner + Vec3::Z * TILE_HEIGHT / 2.0).x / width,
                    (top_left_corner + Vec3::Z * TILE_HEIGHT / 2.0).z / height,
                ),
            },
            // Middle middle
            TileVertex {
                position: top_left_corner + Vec3::new(TILE_WIDTH / 2.0, 0.0, TILE_HEIGHT / 2.0),
                uv: Vec2::new(
                    (top_left_corner + Vec3::new(TILE_WIDTH / 2.0, 0.0, TILE_HEIGHT / 2.0)).x
                        / width,
                    (top_left_corner + Vec3::new(TILE_WIDTH / 2.0, 0.0, TILE_HEIGHT / 2.0)).z
                        / height,
                ),
            },
            // Middle right
            TileVertex {
                position: top_left_corner + Vec3::new(TILE_WIDTH, 0.0, TILE_HEIGHT / 2.0),
                uv: Vec2::new(
                    (top_left_corner + Vec3::new(TILE_WIDTH, 0.0, TILE_HEIGHT / 2.0)).x / width,
                    (top_left_corner + Vec3::new(TILE_WIDTH, 0.0, TILE_HEIGHT / 2.0)).z / height,
                ),
            },
            // Bottom left
            TileVertex {
                position: top_left_corner + Vec3::new(0.0, 0.0, TILE_HEIGHT),
                uv: Vec2::new(
                    (top_left_corner + Vec3::new(0.0, 0.0, TILE_HEIGHT)).x / width,
                    (top_left_corner + Vec3::new(0.0, 0.0, TILE_HEIGHT)).z / height,
                ),
            },
            // Bottom middle
            TileVertex {
                position: top_left_corner + Vec3::new(TILE_WIDTH / 2.0, 0.0, TILE_HEIGHT),
                uv: Vec2::new(
                    (top_left_corner + Vec3::new(TILE_WIDTH / 2.0, 0.0, TILE_HEIGHT)).x / width,
                    (top_left_corner + Vec3::new(TILE_WIDTH / 2.0, 0.0, TILE_HEIGHT)).z / height,
                ),
            },
            // Bottom right
            TileVertex {
                position: top_left_corner + Vec3::new(TILE_WIDTH, 0.0, TILE_HEIGHT),
                uv: Vec2::new(
                    (top_left_corner + Vec3::new(TILE_WIDTH, 0.0, TILE_HEIGHT)).x / width,
                    (top_left_corner + Vec3::new(TILE_WIDTH, 0.0, TILE_HEIGHT)).z / height,
                ),
            },
        ];
        #[rustfmt::skip]
        let indicies = [
            // 1
            start_idx, start_idx + 4, start_idx + 1,
            // 2
            start_idx, start_idx + 3, start_idx + 4,
            // 3
            start_idx + 1, start_idx + 4, start_idx + 2,
            // 4
            start_idx + 2, start_idx + 4, start_idx + 5,
            // 5
            start_idx + 4, start_idx + 3, start_idx + 6,
            // 6
            start_idx + 6, start_idx + 7, start_idx + 4,
            // 7
            start_idx + 4, start_idx + 8, start_idx + 5,
            // 8
            start_idx + 4, start_idx + 7, start_idx + 8,
        ];
        Tile {
            verticies,
            indicies,
            tile_type: TileType::Flat,
            base_height: 0.0,
        }
    }

    fn set_height(&mut self, height: f32) {
        self.base_height = height;
        self.verticies
            .iter_mut()
            .for_each(|vert| vert.position.y = height);
    }
}

#[cfg(feature = "graphics")]
use crate::rendering::*;

#[cfg(feature = "graphics")]
#[derive(Debug)]
// This isn't really needed, could be merged with drawable map
pub struct TileMapRenderData<'a> {
    vertex_buffer: vertex_buffers::MutableVertexData<TileVertex>,
    index_buffer: wgpu::Buffer,
    num_indexes: u32,
    color_layer_texture: wgpu::Texture,
    color_layer_content: texture::TextureContent<'a>,
    decal_layer_texture: wgpu::Texture,
    decal_layer_content: texture::TextureContent<'a>,
    debug_layer_texture: wgpu::Texture,
    debug_layer_content: texture::TextureContent<'a>,
    // TODO remove
    instance_buffer: vertex_buffers::MutableVertexData<crate::rendering::gltf::InstanceData>,
    bind_group: wgpu::BindGroup,
    needs_decal_update: bool,
    needs_color_update: bool,
    needs_debug_update: bool,
    needs_vertex_update: bool,
    tile_width_resultion: u32,
    tile_height_resultion: u32,
}

#[cfg(feature = "graphics")]
impl<'a> TileMapRenderData<'a> {
    // This have no particular meaning anymore
    pub fn decal_buffer_mut(&mut self) -> (u32, &mut [u8]) {
        self.needs_decal_update = true;
        (
            self.decal_layer_content.stride,
            self.decal_layer_content.bytes.to_mut(),
        )
    }

    pub fn color_buffer_mut(&mut self) -> (u32, &mut [u8]) {
        self.needs_color_update = true;
        (
            self.color_layer_content.stride,
            self.color_layer_content.bytes.to_mut(),
        )
    }

    pub fn debug_buffer_mut(&mut self) -> (u32, &mut [u8]) {
        self.needs_debug_update = true;
        (
            self.debug_layer_content.stride,
            self.debug_layer_content.bytes.to_mut(),
        )
    }
}

#[cfg(feature = "graphics")]
impl<'a> TileMapRenderData<'a> {
    fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        tiles: &[Tile],
        size: u32,
        transform: &Transform,
    ) -> Self {
        let resolution = 16;
        let color_layer_content =
            texture::TextureContent::new(size * resolution, size * resolution);
        let color_layer_texture =
            texture::allocate_simple_texture(device, queue, &color_layer_content, true);
        let decal_layer_content =
            texture::TextureContent::new(size * resolution, size * resolution);
        let decal_layer_texture =
            texture::allocate_simple_texture(device, queue, &decal_layer_content, false);
        let debug_layer_content =
            texture::TextureContent::new(size * resolution, size * resolution);
        let debug_layer_texture =
            texture::allocate_simple_texture(device, queue, &decal_layer_content, false);
        //TODO: improve this
        let verticies = tiles
            .into_par_iter()
            .map(|tile| tile.verticies.into_par_iter())
            .flatten()
            .copied()
            .collect::<Vec<TileVertex>>();
        let vertex_buffer =
            vertex_buffers::VertexBuffer::allocate_mutable_buffer(device, &verticies);
        let indicies = tiles
            .into_par_iter()
            .map(|tile| tile.indicies.into_par_iter())
            .flatten()
            .copied()
            .collect::<Vec<u32>>();
        use wgpu::util::DeviceExt;
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Tilemap Index buffer"),
            usage: wgpu::BufferUsage::INDEX,
            contents: bytemuck::cast_slice(&indicies),
        });
        let num_indexes = indicies.len() as u32;
        let instance_buffer = vertex_buffers::VertexBuffer::allocate_mutable_buffer(
            device,
            &[gltf::InstanceData::new(transform.get_model_matrix())],
        );
        let color_view = color_layer_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let decal_layer_view =
            decal_layer_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let debug_layer_view =
            debug_layer_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let color_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Tilemap color texture sampler"),
            address_mode_u: wgpu::AddressMode::ClampToBorder,
            address_mode_v: wgpu::AddressMode::ClampToBorder,
            address_mode_w: wgpu::AddressMode::ClampToBorder,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            lod_min_clamp: -100.0,
            lod_max_clamp: 100.0,
            ..Default::default()
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &tilemap_pass::get_or_create_tilemap_layout(device),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&color_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&decal_layer_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&debug_layer_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&color_sampler),
                },
            ],
            label: Some("Tilemap bindgroup"),
        });
        TileMapRenderData {
            vertex_buffer,
            index_buffer,
            num_indexes,
            color_layer_texture,
            color_layer_content,
            decal_layer_texture,
            decal_layer_content,
            debug_layer_content,
            debug_layer_texture,
            instance_buffer,
            bind_group,
            needs_decal_update: false,
            needs_color_update: false,
            needs_debug_update: false,
            needs_vertex_update: false,
            tile_height_resultion: resolution,
            tile_width_resultion: resolution,
        }
    }
}

fn generate_tiles(size: u32) -> Vec<Tile> {
    let mut tiles = Vec::with_capacity((size * size) as usize);
    let mut index = 0;
    for x in 0..size {
        for z in 0..size {
            tiles.push(Tile::new(
                Vec3::new(TILE_WIDTH * x as f32, 0.0, TILE_HEIGHT * z as f32),
                index,
                size,
            ));
            index += VERTECIES_PER_TILE as u32;
        }
    }
    tiles
}

// Use const generics here for size perhaps
#[derive(Debug, Serialize, Deserialize)]
pub struct TileMap {
    name: String,
    tiles: Vec<Tile>,
    size: u32,
    transform: Transform,
}

impl TileMap {
    pub fn new(name: String, size: u32, transform: Transform) -> Self {
        let tiles = generate_tiles(size);
        TileMap {
            name,
            tiles,
            size,
            transform,
        }
    }

    pub fn load(path: &std::path::Path) -> Result<Self> {
        let map_file = std::fs::File::open(path)?;
        let map = bincode::deserialize_from(map_file)?;
        Ok(map)
    }

    /// Get a reference to the tile map's name.
    #[inline(always)]
    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    /// Get the tile map's size.
    #[inline(always)]
    pub fn size(&self) -> u32 {
        self.size
    }

    /// Get a reference to the tile map's transform.
    pub fn transform(&self) -> &Transform {
        &self.transform
    }

    fn smooth_edges(&mut self, adjacent: &[EdgeAdjacentTile], lowered: bool) {
        // itertools
        let new_height = adjacent
            .iter()
            .filter_map(|adj| {
                if let Some(tile_idx) = adj.tile_idx {
                    self.tiles
                        .get(tile_idx)
                        .map(|tile| tile.verticies[adj.edge as usize].position.y)
                } else {
                    None
                }
            })
            .max_by(|x, y| {
                if lowered {
                    (*y as i32).cmp(&(*x as i32))
                } else {
                    (*x as i32).cmp(&(*y as i32))
                }
            })
            .expect("Tile max corner height couldn't be determined");
        adjacent.iter().for_each(|adj| {
            if let Some(tile_idx) = adj.tile_idx {
                if let Some(height) = self
                    .tiles
                    .get_mut(tile_idx)
                    .map(|tile| &mut tile.verticies[adj.edge as usize].position.y)
                {
                    *height = new_height;
                }
            }
        })
    }

    pub fn set_tile_height(&mut self, x: u32, y: u32, height: f32) {
        // Update
        let is_lowered;
        if let Some(tile) = self.tile_mut(x, y) {
            is_lowered = height < tile.base_height;
            tile.set_height(height);
        } else {
            return;
        }
        let (x, y) = (x as i32, y as i32);
        // marching squares here later on
        let target = self.tile_index(x, y);
        let left = self.tile_index(x - 1, y);
        let right = self.tile_index(x + 1, y);
        let bottom = self.tile_index(x, y + 1);
        // remember 0,0 is the top left corner
        let top = self.tile_index(x, y - 1);
        let top_left = self.tile_index(x - 1, y - 1);
        let top_right = self.tile_index(x + 1, y - 1);
        let bottom_left = self.tile_index(x - 1, y + 1);
        let bottom_right = self.tile_index(x + 1, y + 1);

        // List all vertex adjacent tiles to all of the target tiles' edges
        // top_left_adj refers to all tiles adjacent to the target tiles top_left corner vertex
        let top_left_adj = &[
            EdgeAdjacentTile {
                edge: TileEdge::TopLeft,
                tile_idx: target,
            },
            EdgeAdjacentTile {
                edge: TileEdge::TopRight,
                tile_idx: left,
            },
            EdgeAdjacentTile {
                edge: TileEdge::BottomLeft,
                tile_idx: top,
            },
            EdgeAdjacentTile {
                edge: TileEdge::BottomRight,
                tile_idx: top_left,
            },
        ];
        let top_right_adj = &[
            EdgeAdjacentTile {
                edge: TileEdge::TopLeft,
                tile_idx: right,
            },
            EdgeAdjacentTile {
                edge: TileEdge::TopRight,
                tile_idx: target,
            },
            EdgeAdjacentTile {
                edge: TileEdge::BottomLeft,
                tile_idx: top_right,
            },
            EdgeAdjacentTile {
                edge: TileEdge::BottomRight,
                tile_idx: top,
            },
        ];
        let bottom_left_adj = &[
            EdgeAdjacentTile {
                edge: TileEdge::TopLeft,
                tile_idx: bottom,
            },
            EdgeAdjacentTile {
                edge: TileEdge::TopRight,
                tile_idx: bottom_left,
            },
            EdgeAdjacentTile {
                edge: TileEdge::BottomLeft,
                tile_idx: target,
            },
            EdgeAdjacentTile {
                edge: TileEdge::BottomRight,
                tile_idx: left,
            },
        ];
        let bottom_right_adj = &[
            EdgeAdjacentTile {
                edge: TileEdge::TopLeft,
                tile_idx: bottom_right,
            },
            EdgeAdjacentTile {
                edge: TileEdge::TopRight,
                tile_idx: bottom,
            },
            EdgeAdjacentTile {
                edge: TileEdge::BottomLeft,
                tile_idx: right,
            },
            EdgeAdjacentTile {
                edge: TileEdge::BottomRight,
                tile_idx: target,
            },
        ];
        let middle_left_adj = &[
            EdgeAdjacentTile {
                edge: TileEdge::MiddleRight,
                tile_idx: left,
            },
            EdgeAdjacentTile {
                edge: TileEdge::MiddleLeft,
                tile_idx: target,
            },
        ];
        let middle_right_adj = &[
            EdgeAdjacentTile {
                edge: TileEdge::MiddleLeft,
                tile_idx: right,
            },
            EdgeAdjacentTile {
                edge: TileEdge::MiddleRight,
                tile_idx: target,
            },
        ];
        let middle_top_adj = &[
            EdgeAdjacentTile {
                edge: TileEdge::BottomMiddle,
                tile_idx: top,
            },
            EdgeAdjacentTile {
                edge: TileEdge::TopMiddle,
                tile_idx: target,
            },
        ];
        let middle_bottom_adj = &[
            EdgeAdjacentTile {
                edge: TileEdge::TopMiddle,
                tile_idx: bottom,
            },
            EdgeAdjacentTile {
                edge: TileEdge::BottomMiddle,
                tile_idx: target,
            },
        ];
        self.smooth_edges(top_left_adj, is_lowered);
        self.smooth_edges(top_right_adj, is_lowered);
        self.smooth_edges(bottom_left_adj, is_lowered);
        self.smooth_edges(bottom_right_adj, is_lowered);
        self.smooth_edges(middle_left_adj, is_lowered);
        self.smooth_edges(middle_right_adj, is_lowered);
        self.smooth_edges(middle_top_adj, is_lowered);
        self.smooth_edges(middle_bottom_adj, is_lowered);
    }

    pub fn tile_mut(&mut self, x: u32, y: u32) -> Option<&mut Tile> {
        if let Some(index) = self.tile_index(x as i32, y as i32) {
            self.tiles.get_mut(index)
        } else {
            None
        }
    }

    #[inline(always)]
    /// Returns the index to the given tile where (0,0) corresponds to the top left corner
    pub fn tile_index(&self, x: i32, y: i32) -> Option<usize> {
        if x < 0 || y < 0 {
            None
        } else {
            let (x, y) = (x as u32, y as u32);
            // tiles are column ordered
            Some((x * self.size + y) as usize)
        }
    }
}

#[cfg(feature = "graphics")]
#[derive(Debug)]
pub struct DrawableTileMap<'a> {
    map: TileMap,
    render_data: TileMapRenderData<'a>,
}

#[cfg(feature = "graphics")]
impl<'a> DrawableTileMap<'a> {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, tilemap: TileMap) -> Self {
        let map_size = tilemap.size();
        let render_data = TileMapRenderData::new(
            device,
            queue,
            &tilemap.tiles,
            tilemap.size,
            &tilemap.transform,
        );
        let mut drawable_map = DrawableTileMap {
            map: tilemap,
            render_data,
        };

        // Generate checkered default texture
        // This creates somewhat of an optical illusion (everything looks smaller) which might be
        // undesirable. It's also pretty slow but that can be improved if absolute necessary
        // Also the checkered pattern can in reality be a 4x4 texture that's repeated
        for y in 0..map_size {
            for x in 0..map_size {
                if (x + y) % 2 == 0 {
                    drawable_map.modify_tile_color_texels(x, y, |_, _, buffer| buffer.fill(128));
                } else {
                    drawable_map.modify_tile_color_texels(x, y, |_, _, buffer| buffer.fill(64));
                }
            }
        }
        drawable_map.render_data.needs_color_update = true;
        drawable_map
    }

    /// Get a reference to the tile map's name.
    #[inline(always)]
    pub fn name(&self) -> &str {
        &self.map.name
    }

    #[inline(always)]
    /// Get the tile map's size.
    pub fn size(&self) -> u32 {
        self.map.size
    }

    /// Get a reference to the tile map's transform.
    pub fn transform(&self) -> &Transform {
        &self.map.transform
    }

    pub fn tile_texture_resolution(&self) -> UVec2 {
        UVec2::new(
            self.render_data.tile_width_resultion,
            self.render_data.tile_height_resultion,
        )
    }

    pub fn reset_displacment(&mut self) {
        self.map.tiles = generate_tiles(self.map.size);
        self.render_data.needs_vertex_update = true;
    }

    pub fn reset_color_layer(&mut self) {
        let (_, buffer) = self.render_data.color_buffer_mut();
        buffer.fill(0);
        // Generate checkered default texture
        for y in 0..self.size() {
            for x in 0..self.size() {
                if (x + y) % 2 == 0 {
                    self.modify_tile_color_texels(x, y, |_, _, buffer| buffer.fill(128));
                } else {
                    self.modify_tile_color_texels(x, y, |_, _, buffer| buffer.fill(64));
                }
            }
        }
        self.render_data.needs_color_update = true;
    }

    pub fn reset_decal_layer(&mut self) {
        let (_, buffer) = self.render_data.decal_buffer_mut();
        buffer.fill(0);
        self.render_data.needs_decal_update = true;
    }

    pub fn reset_debug_layer(&mut self) {
        let (_, buffer) = self.render_data.debug_buffer_mut();
        buffer.fill(0);
        self.render_data.needs_debug_update = true;
    }

    pub fn set_tile_height(&mut self, x: u32, y: u32, height: u8) {
        self.map.set_tile_height(x, y, height as f32);
        self.render_data.needs_vertex_update = true;
    }

    pub fn to_tile_coords(&self, world_coords: Vec3A) -> Option<UVec2> {
        let local_coords = self.transform().get_model_matrix().inverse() * world_coords.extend(1.0);
        let map_coords = Vec2::new(local_coords.x / TILE_WIDTH, local_coords.z / TILE_HEIGHT);
        if map_coords.cmplt(Vec2::ZERO).any()
            || map_coords
                .cmpgt(Vec2::new(self.size() as f32, self.size() as f32))
                .any()
        {
            None
        } else {
            let ret = UVec2::new(map_coords.x as u32, map_coords.y as u32);
            Some(ret)
        }
    }

    fn modify_tile_texels<F>(
        tile_x: u32,
        tile_y: u32,
        map_size: u32,
        width_resolution: u32,
        height_resolution: u32,
        stride: u32,
        texel_buffer: &mut [u8],
        func: F,
    ) where
        F: Fn(u32, u32, &mut [u8]) + Send + Sync,
    {
        // texel row size
        let row_size = stride * map_size * width_resolution;
        // only go through the relevant _tile_ row
        let texel_start = (tile_y * row_size * height_resolution) as usize;
        let texel_end = ((tile_y + 1) * row_size * height_resolution) as usize;
        //  Deterimne the texel row offset for the tile
        let row_offset = (tile_x * width_resolution) as usize;
        // The texel row end for the tile
        let row_tile_end = row_offset + width_resolution as usize;
        texel_buffer[texel_start..texel_end]
            .par_chunks_exact_mut(row_size as usize)
            .enumerate()
            .for_each(|(y, texel_row)| {
                texel_row
                    .par_chunks_exact_mut(stride as usize)
                    .enumerate()
                    .filter(|(i, _)| row_offset <= *i && *i < row_tile_end)
                    .for_each(|(x, bytes)| {
                        func(x as u32, y as u32, bytes);
                    });
            });
    }

    /// Modify a specific tiles decal texels, more efficient than looking through the entire buffer
    pub fn modify_tile_decal_texels<F>(&mut self, tile_x: u32, tile_y: u32, func: F)
    where
        F: Fn(u32, u32, &mut [u8]) + Send + Sync,
    {
        let height_resolution = self.render_data.tile_height_resultion;
        let width_resolution = self.render_data.tile_width_resultion;
        let (stride, buffer) = self.render_data.decal_buffer_mut();
        Self::modify_tile_texels(
            tile_x,
            tile_y,
            self.map.size,
            width_resolution,
            height_resolution,
            stride,
            buffer,
            func,
        );
    }

    pub fn modify_tile_color_texels<F>(&mut self, tile_x: u32, tile_y: u32, func: F)
    where
        F: Fn(u32, u32, &mut [u8]) + Send + Sync,
    {
        let height_resolution = self.render_data.tile_height_resultion;
        let width_resolution = self.render_data.tile_width_resultion;
        let (stride, buffer) = self.render_data.color_buffer_mut();
        Self::modify_tile_texels(
            tile_x,
            tile_y,
            self.map.size,
            width_resolution,
            height_resolution,
            stride,
            buffer,
            func,
        );
    }

    /// Modify a tilemap texutre by providing a closure which modifies the texel at the
    /// provided coordinates
    fn modify_texels<F>(
        map_size: u32,
        width_resolution: u32,
        stride: u32,
        texel_buffer: &mut [u8],
        func: F,
    ) where
        F: Fn(u32, u32, &mut [u8]) + Send + Sync,
    {
        // texel row size
        let row_size = stride * map_size * width_resolution;
        texel_buffer
            .par_chunks_exact_mut(row_size as usize)
            .enumerate()
            .for_each(|(y, texel_row)| {
                texel_row
                    .par_chunks_exact_mut(stride as usize)
                    .enumerate()
                    .for_each(|(x, bytes)| func(x as u32, y as u32, bytes))
            });
    }

    /// Modify the decal tilmap texture by providing a closure which modifies the texel at the
    /// provided coordinates
    pub fn modify_decal_texels<F>(&mut self, func: F)
    where
        F: Fn(u32, u32, &mut [u8]) + Send + Sync,
    {
        let width_resolution = self.render_data.tile_width_resultion;
        let (stride, buffer) = self.render_data.decal_buffer_mut();
        Self::modify_texels(self.map.size, width_resolution, stride, buffer, func);
    }

    pub fn modify_color_texels<F>(&mut self, func: F)
    where
        F: Fn(u32, u32, &mut [u8]) + Send + Sync,
    {
        let width_resolution = self.render_data.tile_width_resultion;
        let (stride, buffer) = self.render_data.color_buffer_mut();
        Self::modify_texels(self.map.size, width_resolution, stride, buffer, func);
    }

    //TODO: This doesn't serialize the color texture
    pub fn serialize(&self) -> Result<Vec<u8>> {
        let serialized = bincode::serialize(&self.map)?;
        Ok(serialized)
    }

    pub fn update(&mut self, queue: &wgpu::Queue) {
        if self.render_data.needs_decal_update {
            texture::update_texture_data(
                &self.render_data.decal_layer_content,
                &self.render_data.decal_layer_texture,
                queue,
            );
            self.render_data.needs_decal_update = false;
        }
        if self.render_data.needs_vertex_update {
            let data = self
                .map
                .tiles
                .iter()
                .flat_map(|tile| tile.verticies.iter().copied())
                .collect::<Vec<_>>();
            self.render_data.vertex_buffer.update(queue, &data);
            self.render_data.needs_vertex_update = false;
        }
        if self.render_data.needs_color_update {
            texture::update_texture_data(
                &self.render_data.color_layer_content,
                &self.render_data.color_layer_texture,
                queue,
            );
            self.render_data.needs_color_update = false;
        }
    }

    pub fn draw<'map, 'encoder>(&'map self, render_pass: &mut wgpu::RenderPass<'encoder>)
    where
        'map: 'encoder,
    {
        render_pass.set_bind_group(0, &self.render_data.bind_group, &[]);
        render_pass.set_vertex_buffer(
            0,
            vertex_buffers::VertexBufferData::slice(&self.render_data.vertex_buffer, ..),
        );
        render_pass.set_index_buffer(
            self.render_data.index_buffer.slice(..),
            wgpu::IndexFormat::Uint32,
        );
        render_pass.set_vertex_buffer(
            1,
            vertex_buffers::VertexBufferData::slice(&self.render_data.instance_buffer, ..),
        );
        render_pass.draw_indexed(0..self.render_data.num_indexes, 0, 0..1);
    }
}
