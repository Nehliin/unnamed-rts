use crevice::std430::AsStd430;
use crevice::std430::Std430;
use legion::{world::SubWorld, *};
use once_cell::sync::OnceCell;
use std::sync::atomic::{AtomicU64, Ordering};

const MAX_LIGHTS: i32 = 5;
// alignment differs in array vs outside array
const POINTLIGHT_STD430_ELEMENT_SIZE: usize =
    std::mem::size_of::<<PointLight as AsStd430>::Std430Type>() + 4;
#[derive(Debug, Copy, Clone, AsStd430)]
pub struct PointLight {
    pub color: mint::Vector3<f32>,
    // Let this be defined by transform component instead perhaps?
    pub position: mint::Vector3<f32>,
}

#[derive(Debug, Copy, Clone, AsStd430)]
struct LightCount {
    point_light_count: i32,
}
pub struct LightUniformBuffer {
    pub bind_group: wgpu::BindGroup,
    light_buffer: wgpu::Buffer,
    light_count_buffer: wgpu::Buffer,
}

impl LightUniformBuffer {
    pub fn get_or_create_layout(device: &wgpu::Device) -> &'static wgpu::BindGroupLayout {
        static LAYOUT: OnceCell<wgpu::BindGroupLayout> = OnceCell::new();
        LAYOUT.get_or_init(move || {
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Light uniform buffer layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStage::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    // Light counts
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStage::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            })
        })
    }

    pub fn new(device: &wgpu::Device) -> LightUniformBuffer {
        let light_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Point light buffer"),
            size: (POINTLIGHT_STD430_ELEMENT_SIZE * MAX_LIGHTS as usize) as u64,
            usage: wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
            mapped_at_creation: false,
        });

        let light_count_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Light count buffer"),
            size: LightCount::std430_size_static() as u64,
            usage: wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Light uniform bindgroup"),
            layout: Self::get_or_create_layout(device),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &light_buffer,
                        offset: 0,
                        size: None,
                    }),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &light_count_buffer,
                        offset: 0,
                        size: None,
                    }),
                },
            ],
        });
        LightUniformBuffer {
            bind_group,
            light_buffer,
            light_count_buffer,
        }
    }
}

#[system]
pub fn update(
    world: &SubWorld,
    #[resource] light_uniform: &LightUniformBuffer,
    #[resource] queue: &wgpu::Queue,
    query: &mut Query<&PointLight>,
) {
    let offset = AtomicU64::new(0);
    query.par_for_each(world, |light| {
        queue.write_buffer(
            &light_uniform.light_buffer,
            offset.fetch_add(1, Ordering::AcqRel) * POINTLIGHT_STD430_ELEMENT_SIZE as u64,
            light.as_std430().as_bytes(),
        )
    });
    let offset = offset.load(Ordering::Acquire);
    let light_count = LightCount {
        point_light_count: offset as i32,
    };
    queue.write_buffer(
        &light_uniform.light_count_buffer,
        0,
        light_count.as_std430().as_bytes(),
    );
    debug_assert!(offset < MAX_LIGHTS as u64, "MAX_LIGHTS exceeded");
}
