use bytemuck::{Pod, Zeroable};
use glam::{UVec2, Vec2, Vec3, Vec3A};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use serde::{Deserialize, Serialize};

use crate::components::Transform;
#[repr(C)]
#[derive(Debug, Default, Pod, Zeroable, Deserialize, Serialize, Clone, Copy)]
pub struct TileVertex {
    position: Vec3,
    uv: Vec2,
}
#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
// take f32 per enum val?
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

#[derive(Debug)]
enum Corner {
    TopLeft { vert_idx: u32, tile_idx: usize },
    TopRight { vert_idx: u32, tile_idx: usize },
    BottomLeft { vert_idx: u32, tile_idx: usize },
    BottomRight { vert_idx: u32, tile_idx: usize },
}

impl Default for TileType {
    fn default() -> Self {
        TileType::Flat
    }
}

const VERTECIES_PER_TILE: usize = 9;
const INDICIES_PER_TILE: usize = 24;
pub const TILE_WIDTH: f32 = 2.0;
pub const TILE_HEIGHT: f32 = 2.0;
// Z coords per tile?

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
        // FIX TEXTURE MAPPNG: Maybe use z instead of y?
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
        // TODO: Fix order of these
        let indicies = [
            // 1
            start_idx + 1, start_idx + 4, start_idx,
            // 2
            start_idx + 4, start_idx + 3, start_idx,
            // 3
            start_idx + 2, start_idx + 4, start_idx + 1,
            // 4
            start_idx + 5, start_idx + 4, start_idx + 2,
            // 5
            start_idx + 6, start_idx + 3, start_idx + 4,
            // 6
            start_idx + 4, start_idx + 7, start_idx + 6,
            // 7
            start_idx + 5, start_idx + 8, start_idx + 4,
            // 8
            start_idx + 8, start_idx + 7, start_idx + 4,
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
pub struct TileMapRenderData<'a> {
    pub vertex_buffer: vertex_buffers::MutableVertexData<TileVertex>,
    pub index_buffer: wgpu::Buffer,
    pub num_indexes: u32,
    color_texture: wgpu::Texture,
    color_content: texture::TextureContent<'a>,
    decal_layer_texture: wgpu::Texture,
    decal_layer_content: texture::TextureContent<'a>,
    // TODO remove
    pub instance_buffer: vertex_buffers::MutableVertexData<crate::rendering::gltf::InstanceData>,
    pub bind_group: wgpu::BindGroup,
    pub needs_decal_update: bool,
    pub tile_width_resultion: u32,
    pub tile_height_resultion: u32,
}

#[cfg(feature = "graphics")]
impl<'a> TileMapRenderData<'a> {
    pub fn decal_buffer_mut(&mut self) -> (u32, &mut [u8]) {
        self.needs_decal_update = true;
        (
            self.decal_layer_content.stride,
            self.decal_layer_content.bytes.to_mut(),
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
        let color_content = texture::TextureContent::checkerd(size, resolution as usize);
        let color_texture = texture::allocate_simple_texture(device, queue, &color_content, true);
        let decal_layer_content =
            texture::TextureContent::black(size * resolution as u32, size * resolution as u32);
        let decal_layer_texture =
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
        let color_view = color_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let decal_layer_view =
            decal_layer_texture.create_view(&wgpu::TextureViewDescriptor::default());
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
                    resource: wgpu::BindingResource::Sampler(&color_sampler),
                },
            ],
            label: Some("Tilemap bindgroup"),
        });
        TileMapRenderData {
            vertex_buffer,
            index_buffer,
            num_indexes,
            color_texture,
            color_content,
            decal_layer_texture,
            decal_layer_content,
            instance_buffer,
            bind_group,
            needs_decal_update: false,
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

// Use const generics here perhaps
#[derive(Debug, Serialize, Deserialize)]
pub struct TileMap {
    name: String,
    tiles: Vec<Tile>,
    size: u32,
    transform: Transform,
    needs_vertex_update: bool,
}

impl TileMap {
    pub fn new(name: String, size: u32, transform: Transform) -> Self {
        let tiles = generate_tiles(size);
        TileMap {
            name,
            tiles,
            size,
            transform,
            needs_vertex_update: false,
        }
    }

    pub fn reset(&mut self) {
        self.tiles = generate_tiles(self.size);
        self.needs_vertex_update = true;
    }

    pub fn to_tile_coords(&self, world_coords: Vec3A) -> Option<UVec2> {
        let local_coords = self.transform.get_model_matrix().inverse() * world_coords.extend(1.0);
        let map_coords = Vec2::new(local_coords.x / TILE_WIDTH, local_coords.z / TILE_HEIGHT);
        if map_coords.cmplt(Vec2::ZERO).any()
            || map_coords
                .cmpgt(Vec2::new(self.size as f32, self.size as f32))
                .any()
        {
            None
        } else {
            let ret = UVec2::new(map_coords.x as u32, map_coords.y as u32);
            Some(ret)
        }
    }

    /// Get a reference to the tile map's name.
    #[inline(always)]
    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    /// Get a reference to the tile map's size.
    #[inline(always)]
    pub fn size(&self) -> u32 {
        self.size
    }

    /// Get a reference to the tile map's transform.
    pub fn transform(&self) -> &Transform {
        &self.transform
    }

    fn smooth_corners(&mut self, tiles: &[Corner], lowered: bool) {
        // itertools
        let new_height = tiles
            .iter()
            .filter_map(|corner| match corner {
                &Corner::TopLeft { vert_idx, tile_idx } => self
                    .tiles
                    .get(tile_idx)
                    .map(|tile| tile.verticies[vert_idx as usize].position.y),
                &Corner::TopRight { vert_idx, tile_idx } => self
                    .tiles
                    .get(tile_idx)
                    .map(|tile| tile.verticies[vert_idx as usize].position.y),
                &Corner::BottomLeft { vert_idx, tile_idx } => self
                    .tiles
                    .get(tile_idx)
                    .map(|tile| tile.verticies[vert_idx as usize].position.y),
                &Corner::BottomRight { vert_idx, tile_idx } => self
                    .tiles
                    .get(tile_idx)
                    .map(|tile| tile.verticies[vert_idx as usize].position.y),
            })
            .max_by(|x, y| {
                if lowered {
                    (*y as u32).cmp(&(*x as u32))
                } else {
                    (*x as u32).cmp(&(*y as u32))
                }
            })
            .unwrap();
        tiles
            .iter()
            .filter_map(|corner| match corner {
                &Corner::TopLeft { vert_idx, tile_idx } => self
                    .tiles
                    .get_mut(tile_idx)
                    .map(|tile| &mut tile.verticies[vert_idx as usize].position.y as *mut f32),
                &Corner::TopRight { vert_idx, tile_idx } => self
                    .tiles
                    .get_mut(tile_idx)
                    .map(|tile| &mut tile.verticies[vert_idx as usize].position.y as *mut f32),
                &Corner::BottomLeft { vert_idx, tile_idx } => self
                    .tiles
                    .get_mut(tile_idx)
                    .map(|tile| &mut tile.verticies[vert_idx as usize].position.y as *mut f32),
                &Corner::BottomRight { vert_idx, tile_idx } => self
                    .tiles
                    .get_mut(tile_idx)
                    .map(|tile| &mut tile.verticies[vert_idx as usize].position.y as *mut f32),
            })
            .for_each(|height| unsafe { *height = new_height as f32 })
    }

    pub fn set_tile_heght(&mut self, x: u32, y: u32, height: f32, lowered: bool) {
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

        // Update
        if let Some(tile) = self.tile_mut(x, y) {
            tile.verticies
                .iter_mut()
                .for_each(|vertex| vertex.position.y = height);
            self.needs_vertex_update = true;
        } else {
            return;
        }
        // smooth out corners
        let top_left_tiles = &[
            Corner::TopLeft {
                vert_idx: 0,
                tile_idx: target,
            },
            Corner::TopRight {
                vert_idx: 2,
                tile_idx: left,
            },
            Corner::BottomLeft {
                vert_idx: 6,
                tile_idx: top,
            },
            Corner::BottomRight {
                vert_idx: 8,
                tile_idx: top_left,
            },
        ];
        let top_right_tiles = &[
            Corner::TopLeft {
                vert_idx: 0,
                tile_idx: right,
            },
            Corner::TopRight {
                vert_idx: 2,
                tile_idx: target,
            },
            Corner::BottomLeft {
                vert_idx: 6,
                tile_idx: top_right,
            },
            Corner::BottomRight {
                vert_idx: 8,
                tile_idx: top,
            },
        ];
        let bottom_left_tiles = &[
            Corner::TopLeft {
                vert_idx: 0,
                tile_idx: bottom,
            },
            Corner::TopRight {
                vert_idx: 2,
                tile_idx: bottom_left,
            },
            Corner::BottomLeft {
                vert_idx: 6,
                tile_idx: target,
            },
            Corner::BottomRight {
                vert_idx: 8,
                tile_idx: left,
            },
        ];
        let bottom_right_tiles = &[
            Corner::TopLeft {
                vert_idx: 0,
                tile_idx: bottom_right,
            },
            Corner::TopRight {
                vert_idx: 2,
                tile_idx: bottom,
            },
            Corner::BottomLeft {
                vert_idx: 6,
                tile_idx: right,
            },
            Corner::BottomRight {
                vert_idx: 8,
                tile_idx: target,
            },
        ];
        self.smooth_corners(top_left_tiles, lowered);
        self.smooth_corners(top_right_tiles, lowered);
        self.smooth_corners(bottom_left_tiles, lowered);
        self.smooth_corners(bottom_right_tiles, lowered);
        let middle_height = |x: f32, y: f32| {
            if lowered {
                std::cmp::min(x as i32, y as i32)
            } else {
                std::cmp::max(x as i32, y as i32)
            }
        };
        // smooth out middle points
        let mid_left = &mut self.tiles[left].verticies[5].position.y;
        *mid_left = middle_height(*mid_left, height) as f32;
        let mid_right = &mut self.tiles[right].verticies[3].position.y;
        *mid_right = middle_height(*mid_right, height) as f32;
        let top_mid = &mut self.tiles[top].verticies[7].position.y;
        *top_mid = middle_height(*top_mid, height) as f32;
        let bottom_mid = &mut self.tiles[bottom].verticies[1].position.y;
        *bottom_mid = middle_height(*bottom_mid, height) as f32;
        //TODO clean this method up
    }

    pub fn tile_mut(&mut self, x: u32, y: u32) -> Option<&mut Tile> {
        self.needs_vertex_update = true;
        let index = self.tile_index(x, y);
        self.tiles.get_mut(index)
    }

    #[inline(always)]
    /// Returns the index to the given tile where (0,0) corresponds to the top left corner
    pub fn tile_index(&self, x: u32, y: u32) -> usize {
        // tiles are column ordered
        (x * self.size + y) as usize
    }
}
#[derive(Debug)]
#[cfg(feature = "graphics")]
pub struct DrawableTileMap<'a> {
    pub map: TileMap,
    pub render_data: TileMapRenderData<'a>,
}

#[cfg(feature = "graphics")]
impl<'a> DrawableTileMap<'a> {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, tilemap: TileMap) -> Self {
        let render_data = TileMapRenderData::new(
            device,
            queue,
            &tilemap.tiles,
            tilemap.size,
            &tilemap.transform,
        );
        DrawableTileMap {
            map: tilemap,
            render_data,
        }
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
        if self.map.needs_vertex_update {
            let data = self
                .map
                .tiles
                .iter()
                .flat_map(|tile| tile.verticies.iter().copied())
                .collect::<Vec<_>>();
            self.render_data.vertex_buffer.update(queue, &data);
            self.map.needs_vertex_update = false;
        }
    }
}
