use crate::{
    map_chunk::{ChunkIndex, MapChunk, CHUNK_SIZE},
    rendering::{pass::tilemap_pass, *},
    tilemap::*,
};
use anyhow::Result;
use glam::{UVec2, Vec2, Vec3A};
use rayon::{
    iter::{IndexedParallelIterator, IntoParallelIterator, ParallelIterator},
    slice::ParallelSliceMut,
};

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
    instance_buffer: vertex_buffers::MutableVertexData<gltf::InstanceData>,
    bind_group: wgpu::BindGroup,
    needs_decal_update: bool,
    needs_color_update: bool,
    needs_debug_update: bool,
    needs_vertex_update: bool,
    tile_width_resultion: u32,
    tile_height_resultion: u32,
}

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

impl<'a> TileMapRenderData<'a> {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue, grid: &MapChunk<Tile>) -> Self {
        let resolution = 16;
        let color_layer_content = texture::TextureContent::new(
            CHUNK_SIZE as u32 * resolution,
            CHUNK_SIZE as u32 * resolution,
        );
        let color_layer_texture =
            texture::allocate_simple_texture(device, queue, &color_layer_content, true);
        let decal_layer_content = texture::TextureContent::new(
            CHUNK_SIZE as u32 * resolution,
            CHUNK_SIZE as u32 * resolution,
        );
        let decal_layer_texture =
            texture::allocate_simple_texture(device, queue, &decal_layer_content, false);
        let debug_layer_content = texture::TextureContent::new(
            CHUNK_SIZE as u32 * resolution,
            CHUNK_SIZE as u32 * resolution,
        );
        let debug_layer_texture =
            texture::allocate_simple_texture(device, queue, &decal_layer_content, false);
        //TODO: improve this
        let verticies = grid
            .tiles()
            .into_par_iter()
            .map(|tile| tile.verticies.into_par_iter())
            .flatten()
            //.copied()
            .collect::<Vec<TileVertex>>();
        let vertex_buffer =
            vertex_buffers::VertexBuffer::allocate_mutable_buffer(device, &verticies);
        let indicies = grid
            .tiles()
            .into_par_iter()
            .map(|tile| tile.indicies.into_par_iter())
            .flatten()
            //.copied()
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
            &[gltf::InstanceData::new(grid.transform())],
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
            layout: tilemap_pass::get_or_create_tilemap_layout(device),
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

#[derive(Debug)]
pub struct DrawableTileMap<'a> {
    map: TileMap,
    render_data: TileMapRenderData<'a>,
}

impl<'a> DrawableTileMap<'a> {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, tilemap: TileMap) -> Self {
        let render_data = TileMapRenderData::new(device, queue, &tilemap.chunk);
        let mut drawable_map = DrawableTileMap {
            map: tilemap,
            render_data,
        };

        // Generate checkered default texture
        // This creates somewhat of an optical illusion (everything looks smaller) which might be
        // undesirable. It's also pretty slow but that can be improved if absolute necessary
        // Also the checkered pattern can in reality be a 4x4 texture that's repeated
        for y in 0..CHUNK_SIZE as u32 {
            for x in 0..CHUNK_SIZE as u32 {
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

    /// Get a reference to the underlying MapChunk.
    #[inline]
    pub fn tile_grid(&self) -> &MapChunk<Tile> {
        &self.map.chunk
    }

    /// Get a reference to the tile map's name.
    #[inline(always)]
    pub fn name(&self) -> &str {
        &self.map.name
    }

    #[inline]
    pub fn tile_texture_resolution(&self) -> UVec2 {
        UVec2::new(
            self.render_data.tile_width_resultion,
            self.render_data.tile_height_resultion,
        )
    }

    #[inline]
    pub fn reset_displacment(&mut self) {
        self.map.chunk = generate_grid(*self.tile_grid().transform());
        self.render_data.needs_vertex_update = true;
    }

    pub fn reset_color_layer(&mut self) {
        let (_, buffer) = self.render_data.color_buffer_mut();
        buffer.fill(0);
        // Generate checkered default texture
        for y in 0..CHUNK_SIZE as u32 {
            for x in 0..CHUNK_SIZE as u32 {
                if (x + y) % 2 == 0 {
                    self.modify_tile_color_texels(x, y, |_, _, buffer| buffer.fill(128));
                } else {
                    self.modify_tile_color_texels(x, y, |_, _, buffer| buffer.fill(64));
                }
            }
        }
        self.render_data.needs_color_update = true;
    }

    #[inline]
    pub fn reset_decal_layer(&mut self) {
        let (_, buffer) = self.render_data.decal_buffer_mut();
        buffer.fill(0);
        self.render_data.needs_decal_update = true;
    }

    #[inline]
    pub fn reset_debug_layer(&mut self) {
        let (_, buffer) = self.render_data.debug_buffer_mut();
        buffer.fill(0);
        self.render_data.needs_debug_update = true;
    }

    pub fn fill_debug_layer(&mut self) {
        let height_resolution = self.render_data.tile_height_resultion;
        let width_resolution = self.render_data.tile_width_resultion;
        for x in 0..CHUNK_SIZE {
            for y in 0..CHUNK_SIZE {
                let index = ChunkIndex::new(x, y).expect("For loops doesn't match grid size");
                let tile = self.tile_grid().tile(index);
                match tile.tile_type {
                    TileType::Flat => {}
                    TileType::RampTop
                    | TileType::RampBottom
                    | TileType::RampRight
                    | TileType::RampLeft => {
                        if tile.height_diff.abs() < 2 {
                            {
                                let (stride, buffer) = self.render_data.debug_buffer_mut();
                                Self::modify_tile_texels(
                                    x as u32,
                                    y as u32,
                                    width_resolution,
                                    height_resolution,
                                    stride,
                                    buffer,
                                    |_, _, tile_texels| {
                                        tile_texels[2] = 255;
                                        tile_texels[3] = 255;
                                    },
                                );
                            }
                        }
                    }
                    _ => {
                        let (stride, buffer) = self.render_data.debug_buffer_mut();
                        Self::modify_tile_texels(
                            x as u32,
                            y as u32,
                            width_resolution,
                            height_resolution,
                            stride,
                            buffer,
                            |_, _, tile_texels| {
                                tile_texels[0] = 255;
                                tile_texels[3] = 255;
                            },
                        );
                    }
                }
            }
        }
        self.render_data.needs_debug_update = true;
    }

    #[inline]
    pub fn set_tile_height(&mut self, x: i32, y: i32, height: u8) {
        self.map.set_tile_height(x, y, height as f32);
        self.render_data.needs_vertex_update = true;
    }

    #[inline]
    pub fn tile(&self, x: i32, y: i32) -> Option<&Tile> {
        ChunkIndex::new(x, y)
            .ok()
            .map(|idx| self.map.chunk.tile(idx))
    }

    pub fn to_tile_coords(&self, world_coords: Vec3A) -> Option<UVec2> {
        let local_coords = self
            .tile_grid()
            .transform()
            .matrix
            .inverse()
            .transform_point3a(world_coords.extend(1.0).into());
        let map_coords = Vec2::new(local_coords.x / TILE_WIDTH, local_coords.z / TILE_HEIGHT);
        if map_coords.cmplt(Vec2::ZERO).any()
            || map_coords
                .cmpgt(Vec2::new(CHUNK_SIZE as f32, CHUNK_SIZE as f32))
                .any()
        {
            None
        } else {
            let ret = UVec2::new(map_coords.x as u32, map_coords.y as u32);
            Some(ret)
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn modify_tile_texels<F>(
        tile_x: u32,
        tile_y: u32,
        width_resolution: u32,
        height_resolution: u32,
        stride: u32,
        texel_buffer: &mut [u8],
        func: F,
    ) where
        F: Fn(u32, u32, &mut [u8]) + Send + Sync,
    {
        // texel row size
        let row_size = stride * CHUNK_SIZE as u32 * width_resolution;
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
            width_resolution,
            height_resolution,
            stride,
            buffer,
            func,
        );
    }

    pub fn modify_tile_debug_texels<F>(&mut self, tile_x: u32, tile_y: u32, func: F)
    where
        F: Fn(u32, u32, &mut [u8]) + Send + Sync,
    {
        let height_resolution = self.render_data.tile_height_resultion;
        let width_resolution = self.render_data.tile_width_resultion;
        let (stride, buffer) = self.render_data.debug_buffer_mut();
        Self::modify_tile_texels(
            tile_x,
            tile_y,
            width_resolution,
            height_resolution,
            stride,
            buffer,
            func,
        );
    }

    /// Modify a tilemap texutre by providing a closure which modifies the texel at the
    /// provided coordinates
    fn modify_texels<F>(width_resolution: u32, stride: u32, texel_buffer: &mut [u8], func: F)
    where
        F: Fn(u32, u32, &mut [u8]) + Send + Sync,
    {
        // texel row size
        let row_size = stride * CHUNK_SIZE as u32 * width_resolution;
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
        Self::modify_texels(width_resolution, stride, buffer, func);
    }

    pub fn modify_color_texels<F>(&mut self, func: F)
    where
        F: Fn(u32, u32, &mut [u8]) + Send + Sync,
    {
        let width_resolution = self.render_data.tile_width_resultion;
        let (stride, buffer) = self.render_data.color_buffer_mut();
        Self::modify_texels(width_resolution, stride, buffer, func);
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
                .chunk
                .tiles()
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
        if self.render_data.needs_debug_update {
            texture::update_texture_data(
                &self.render_data.debug_layer_content,
                &self.render_data.debug_layer_texture,
                queue,
            );
            self.render_data.needs_debug_update = false;
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
