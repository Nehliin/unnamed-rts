use bytemuck::{Pod, Zeroable};
use glam::{Vec2, Vec3};
use rayon::iter::{IntoParallelIterator, ParallelIterator};

use crate::components::Transform;
#[repr(C)]
#[derive(Debug, Default, Pod, Zeroable, Clone, Copy)]
pub struct TileVertex {
    position: Vec3,
    uv: Vec2,
}
#[derive(Debug, Clone, Copy)]
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
// X/Y/Z coords per tile?

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
#[derive(Debug, Clone, Copy, Default)]
pub struct Tile {
    verticies: [TileVertex; VERTECIES_PER_TILE],
    indicies: [u32; INDICIES_PER_TILE],
    tile_type: TileType,
    // base_height, ramp_heigh
}

impl Tile {
    pub fn new(top_left_corner: Vec3, start_idx: u32, size: u32) -> Self {
        let size = size as f32;
        // TODO change order of this to make indices closer to each other
        // FIX TEXTURE MAPPNG: Maybe use z instead of y?
        let verticies = [
            // Top left
            TileVertex {
                position: top_left_corner,
                uv: Vec2::new(top_left_corner.x / size, top_left_corner.z / size),
            },
            // Top middle
            TileVertex {
                position: top_left_corner + Vec3::X * TILE_WIDTH / 2.0,
                uv: Vec2::new(
                    (top_left_corner + Vec3::X * TILE_WIDTH / 2.0).x / size,
                    (top_left_corner + Vec3::X * TILE_WIDTH / 2.0).z / size,
                ),
            },
            // Top right
            TileVertex {
                position: top_left_corner + Vec3::X * TILE_WIDTH,
                uv: Vec2::new(
                    (top_left_corner + Vec3::X * TILE_WIDTH).x / size,
                    (top_left_corner + Vec3::X * TILE_WIDTH).z / size,
                ),
            },
            // Middle left
            TileVertex {
                position: top_left_corner - Vec3::Z * TILE_HEIGHT / 2.0,
                uv: Vec2::new(
                    (top_left_corner - Vec3::Z * TILE_HEIGHT / 2.0).x / size,
                    (top_left_corner - Vec3::Z * TILE_HEIGHT / 2.0).z / size,
                ),
            },
            // Middle middle
            TileVertex {
                position: top_left_corner + Vec3::new(TILE_WIDTH / 2.0, 0.0, -TILE_HEIGHT / 2.0),
                uv: Vec2::new(
                    (top_left_corner + Vec3::new(TILE_WIDTH / 2.0, 0.0, -TILE_HEIGHT / 2.0)).x
                        / size,
                    (top_left_corner + Vec3::new(TILE_WIDTH / 2.0, 0.0, -TILE_HEIGHT / 2.0)).z
                        / size,
                ),
            },
            // Middle right
            TileVertex {
                position: top_left_corner + Vec3::new(TILE_WIDTH, 0.0, -TILE_HEIGHT / 2.0),
                uv: Vec2::new(
                    (top_left_corner + Vec3::new(TILE_WIDTH, 0.0, -TILE_HEIGHT / 2.0)).x / size,
                    (top_left_corner + Vec3::new(TILE_WIDTH, 0.0, -TILE_HEIGHT / 2.0)).z / size,
                ),
            },
            // Bottom left
            TileVertex {
                position: top_left_corner + Vec3::new(0.0, 0.0, -TILE_HEIGHT),
                uv: Vec2::new(
                    (top_left_corner + Vec3::new(0.0, 0.0, -TILE_HEIGHT)).x / size,
                    (top_left_corner + Vec3::new(0.0, 0.0, -TILE_HEIGHT)).z / size,
                ),
            },
            // Bottom middle
            TileVertex {
                position: top_left_corner + Vec3::new(TILE_WIDTH / 2.0, 0.0, -TILE_HEIGHT),
                uv: Vec2::new(
                    (top_left_corner + Vec3::new(TILE_WIDTH / 2.0, 0.0, -TILE_HEIGHT)).x / size,
                    (top_left_corner + Vec3::new(TILE_WIDTH / 2.0, 0.0, -TILE_HEIGHT)).z / size,
                ),
            },
            // Bottom right
            TileVertex {
                position: top_left_corner + Vec3::new(TILE_WIDTH, 0.0, -TILE_HEIGHT),
                uv: Vec2::new(
                    (top_left_corner + Vec3::new(TILE_WIDTH, 0.0, -TILE_HEIGHT)).x / size,
                    (top_left_corner + Vec3::new(TILE_WIDTH, 0.0, -TILE_HEIGHT)).z / size,
                ),
            },
        ];
        #[rustfmt::skip]
        // CCW ordering
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
        }
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
    pub color_texture: wgpu::Texture,
    pub color_content: texture::TextureContent<'a>,
    // TODO remove
    pub instance_buffer: vertex_buffers::MutableVertexData<crate::rendering::gltf::InstanceData>,
    pub bind_group: wgpu::BindGroup,
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
        let color_content = texture::TextureContent::checkerd(size);
        let color_texture = texture::allocate_simple_texture(device, queue, &color_content, true);
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
        let color_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Tilemap color texture sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
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
            instance_buffer,
            bind_group,
        }
    }
}

// Use const generics here perhaps
#[derive(Debug)]
#[cfg(feature = "graphics")]
pub struct TileMap<'a> {
    pub tiles: Vec<Tile>,
    pub size: u32,
    pub transform: Transform,
    pub render_data: TileMapRenderData<'a>,
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

#[cfg(feature = "graphics")]
impl<'a> TileMap<'a> {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        size: u32,
        transform: Transform,
    ) -> Self {
        let tiles = generate_tiles(size);
        TileMap {
            transform,
            size,
            render_data: TileMapRenderData::new(device, queue, &tiles, size, &transform),
            tiles,
        }
    }
}

#[cfg(not(feature = "graphics"))]
pub struct TileMap {
    tiles: Vec<Tile>,
    size: u32,
    transform: Transform,
}

#[cfg(not(feature = "graphics"))]
impl TileMap {
    pub fn new(size: u32, transform: Transform) -> Self {
        let tiles = generate_tiles(size);
        TileMap {
            size,
            transform,
            tiles,
        }
    }
}
