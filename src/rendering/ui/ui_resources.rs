use crate::{
    assets::{AssetLoader, Handle},
    rendering::texture::{allocate_simple_texture, TextureContent},
    resources::WindowSize,
};
use anyhow::anyhow;
use anyhow::Result;
use egui::{vec2, CtxRef, RawInput};
use image::{GenericImageView, ImageFormat};
use once_cell::sync::OnceCell;
use std::{borrow::Cow, convert::TryFrom, fs::File, io::BufReader, path::Path};
use winit::event::ModifiersState;

pub struct CursorPosition {
    pub x: f64,
    pub y: f64,
}

pub struct UiContext {
    pub context: CtxRef,
    pub raw_input: RawInput,
    pub cursor_pos: CursorPosition,
    pub modifier_state: ModifiersState,
}

impl UiContext {
    pub fn new(window_size: &WindowSize) -> UiContext {
        let context = CtxRef::default();
        let raw_input = egui::RawInput {
            pixels_per_point: Some(window_size.scale_factor),
            screen_rect: Some(egui::Rect::from_min_size(
                Default::default(),
                vec2(
                    window_size.physical_width as f32,
                    window_size.physical_height as f32,
                ) / window_size.scale_factor,
            )),
            ..Default::default()
        };

        UiContext {
            context,
            raw_input,
            cursor_pos: CursorPosition { x: 0.0, y: 0.0 },
            modifier_state: ModifiersState::empty(),
        }
    }
}

#[derive(Debug)]
pub struct UiTexture<'a> {
    pub content: TextureContent<'a>,
    pub bind_group: wgpu::BindGroup,
}

impl<'a> UiTexture<'a> {
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        label: &str,
        content: TextureContent<'a>,
    ) -> Self {
        let texture = allocate_simple_texture(device, queue, &content, true);

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(format!("{}_texture_bind_group", label).as_str()),
            layout: UiTexture::get_or_create_layout(device),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(
                    &texture.create_view(&wgpu::TextureViewDescriptor::default()),
                ),
            }],
        });

        UiTexture {
            content,
            bind_group,
        }
    }

    pub fn get_or_create_layout(device: &wgpu::Device) -> &'static wgpu::BindGroupLayout {
        static LAYOUT: OnceCell<wgpu::BindGroupLayout> = OnceCell::new();
        LAYOUT.get_or_init(move || {
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("UiTexture bindgroup layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStage::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                }],
            })
        })
    }
}

impl From<Handle<UiTexture<'_>>> for egui::TextureId {
    fn from(handle: Handle<UiTexture>) -> Self {
        egui::TextureId::User(handle.get_id() as u64)
    }
}

// bit of a hack
impl TryFrom<egui::TextureId> for Handle<UiTexture<'_>> {
    type Error = anyhow::Error;
    fn try_from(texture_id: egui::TextureId) -> Result<Self> {
        match texture_id {
            egui::TextureId::Egui => Err(anyhow!("No handle exists for Egui textures")),
            egui::TextureId::User(id) => Ok(unsafe { Handle::new_raw_handle(u32::try_from(id)?) }),
        }
    }
}

impl AssetLoader for UiTexture<'_> {
    fn load(path: &Path, device: &wgpu::Device, queue: &wgpu::Queue) -> Result<Self> {
        let file = File::open(path)?;
        let image = image::load(BufReader::new(file), ImageFormat::Png)?;
        let (width, height) = image.dimensions();
        let content = TextureContent {
            label: Some("Ui user texture"),
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            bytes: Cow::Owned(
                image
                    .as_rgba8()
                    .expect("UiTexture couldn't be converted to Rgba8")
                    .to_vec(),
            ),
            stride: 4,
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        };
        Ok(UiTexture::new(device, queue, "custom_ui", content))
    }

    fn extensions() -> &'static [&'static str] {
        // extend this
        &["png"]
    }
}
