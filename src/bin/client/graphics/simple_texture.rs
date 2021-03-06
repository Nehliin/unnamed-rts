use anyhow::Result;
use image::GenericImageView;
use once_cell::sync::OnceCell;
use std::marker::PhantomData;
use texture::{LoadableTexture, TextureData, TextureShaderLayout};
use wgpu::TextureViewDescriptor;

use super::texture;

#[derive(Debug)]
pub struct SimpleTexture;

impl TextureShaderLayout for SimpleTexture {
    const VISIBILITY: wgpu::ShaderStage = wgpu::ShaderStage::FRAGMENT;
    fn get_layout(device: &wgpu::Device) -> &'static wgpu::BindGroupLayout {
        static LAYOUT: OnceCell<wgpu::BindGroupLayout> = OnceCell::new();
        LAYOUT.get_or_init(move || {
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: Self::VISIBILITY,
                        ty: wgpu::BindingType::Texture {
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: Self::VISIBILITY,
                        ty: wgpu::BindingType::Sampler {
                            comparison: false,
                            filtering: true,
                        },
                        count: None,
                    },
                ],
                label: None,
            })
        })
    }
}

impl LoadableTexture for SimpleTexture {
    fn load_texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        path: impl AsRef<std::path::Path>,
    ) -> Result<TextureData<Self>> {
        let img = image::open(path)?;
        let img = img.flipv();

        let rgba = img.to_rgba8(); // handle formats properly
        let (width, height) = img.dimensions();

        let size = wgpu::Extent3d {
            width,
            height,
            depth: 1,
        };

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb, // handle formats properly
            usage: wgpu::TextureUsage::SAMPLED | wgpu::TextureUsage::COPY_DST,
        });

        let texutre_copy_view = wgpu::TextureCopyView {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
        };
        let texture_data_layout = wgpu::TextureDataLayout {
            offset: 0,
            bytes_per_row: 4 * width,
            rows_per_image: 0,
        };

        queue.write_texture(texutre_copy_view, &rgba.into_raw(), texture_data_layout, size);

        let view = texture.create_view(&TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: None,
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            lod_min_clamp: -100.0, // related to mipmaps
            lod_max_clamp: 100.0,  // related to mipmaps
            compare: None,
            ..Default::default()
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &Self::get_layout(device),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
            label: Some("SimpleTextureBindGroup"),
        });
        let texture_data = TextureData {
            bind_group,
            sampler,
            views: vec![view],
            texture,
            _marker: PhantomData::default(),
        };
        Ok(texture_data)
    }
}
