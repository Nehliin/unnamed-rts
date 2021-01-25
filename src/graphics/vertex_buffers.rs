use std::{any::type_name, marker::PhantomData, ops::RangeBounds};
use crevice::std140::{AsStd140};
use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    BufferAddress,
};

pub trait VertexBufferData {
    type DataType: VertexBuffer;
    fn get_gpu_buffer(&self) -> &wgpu::Buffer;
    fn slice<S: RangeBounds<BufferAddress>>(&self, bounds: S) -> wgpu::BufferSlice;
}

pub struct ImmutableVertexData<T: AsStd140> {
    pub(crate) buffer: wgpu::Buffer,
    _marker: PhantomData<T>,
}

pub struct MutableVertexData<T: AsStd140> {
    pub(crate) buffer: wgpu::Buffer,
    _marker: PhantomData<T>,
}

impl<T: VertexBuffer> VertexBufferData for ImmutableVertexData<T> {
    type DataType = T;

    fn get_gpu_buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }

    fn slice<S: RangeBounds<BufferAddress>>(&self, bounds: S) -> wgpu::BufferSlice {
        self.buffer.slice(bounds)
    }
}

impl<T: VertexBuffer> VertexBufferData for MutableVertexData<T> {
    type DataType = T;

    fn get_gpu_buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }

    fn slice<S: RangeBounds<BufferAddress>>(&self, bounds: S) -> wgpu::BufferSlice {
        self.buffer.slice(bounds)
    }
}

impl<T: VertexBuffer> MutableVertexData<T> {
    #[allow(dead_code)]
    pub fn update(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        buffer_data: &[T],
    ) {
        let raw_bytes: Vec<T::Std140Type> = buffer_data.iter().map(|item| item.as_std140()).collect();
        let staging_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("staging buffer"),
            usage: wgpu::BufferUsage::COPY_SRC,
            contents: bytemuck::cast_slice(&raw_bytes),
        });
        encoder.copy_buffer_to_buffer(&staging_buffer, 0, &self.buffer, 0, raw_bytes.len() as u64);
    }
}

pub trait VertexBuffer: AsStd140 + Sized {
    const STEP_MODE: wgpu::InputStepMode;

    fn allocate_immutable_buffer(
        device: &wgpu::Device,
        buffer_data: &[Self],
    ) -> ImmutableVertexData<Self> {
        let raw_bytes: Vec<Self::Std140Type> = buffer_data.iter().map(|item| item.as_std140()).collect();
        ImmutableVertexData {
            _marker: PhantomData::default(),
            buffer: device.create_buffer_init(&BufferInitDescriptor {
                // TODO should only be part of debug builds probably
                label: Some(&format!("Immutable buffer of: {}", type_name::<Self>())),
                usage: wgpu::BufferUsage::VERTEX,
                contents: bytemuck::cast_slice(&raw_bytes),
            }),
        }
    }

    fn allocate_mutable_buffer(
        device: &wgpu::Device,
        buffer_data: &[Self],
    ) -> MutableVertexData<Self> {
        let raw_bytes: Vec<Self::Std140Type> = buffer_data.iter().map(|item| item.as_std140()).collect();
        MutableVertexData {
            _marker: PhantomData::default(),
            buffer: device.create_buffer_init(&BufferInitDescriptor {
                // TODO should only be part of debug builds probably
                label: Some(&format!("Mutable buffer of: {}", type_name::<Self>())),
                usage: wgpu::BufferUsage::VERTEX | wgpu::BufferUsage::COPY_DST,
                contents: bytemuck::cast_slice(&raw_bytes),
            }),
        }
    }

    fn get_descriptor<'a>() -> wgpu::VertexBufferDescriptor<'a> {
        wgpu::VertexBufferDescriptor {
            stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: Self::STEP_MODE,
            attributes: Self::get_attributes(),
        }
    }

    fn get_attributes<'a>() -> &'a [wgpu::VertexAttributeDescriptor];
}
