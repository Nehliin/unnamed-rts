use bytemuck::{Pod, Zeroable};
use std::{any::type_name, marker::PhantomData, ops::RangeBounds};
use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    BufferAddress,
};

pub trait VertexBufferData {
    type DataType: VertexBuffer;
    fn get_gpu_buffer(&self) -> &wgpu::Buffer;
    fn slice<S: RangeBounds<BufferAddress>>(&self, bounds: S) -> wgpu::BufferSlice;
}

pub struct ImmutableVertexData<T: Pod + Zeroable> {
    pub(crate) buffer: wgpu::Buffer,
    _marker: PhantomData<T>,
}

pub struct MutableVertexData<T: Pod + Zeroable> {
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
    pub fn update(&self, queue: &wgpu::Queue, buffer_data: &[T]) {
        queue.write_buffer(&self.buffer, 0, bytemuck::cast_slice(&buffer_data));
    }
}

pub trait VertexBuffer: Pod + Zeroable {
    const STEP_MODE: wgpu::InputStepMode;

    fn allocate_immutable_buffer(
        device: &wgpu::Device,
        buffer_data: &[Self],
    ) -> ImmutableVertexData<Self> {
        ImmutableVertexData {
            _marker: PhantomData::default(),
            buffer: device.create_buffer_init(&BufferInitDescriptor {
                label: Some(&format!("Immutable buffer of: {}", type_name::<Self>())),
                usage: wgpu::BufferUsage::VERTEX,
                contents: bytemuck::cast_slice(&buffer_data),
            }),
        }
    }

    fn allocate_mutable_buffer(
        device: &wgpu::Device,
        buffer_data: &[Self],
    ) -> MutableVertexData<Self> {
        MutableVertexData {
            _marker: PhantomData::default(),
            buffer: device.create_buffer_init(&BufferInitDescriptor {
                label: Some(&format!("Mutable buffer of: {}", type_name::<Self>())),
                usage: wgpu::BufferUsage::VERTEX | wgpu::BufferUsage::COPY_DST,
                contents: bytemuck::cast_slice(&buffer_data),
            }),
        }
    }

    fn get_descriptor<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: Self::STEP_MODE,
            attributes: Self::get_attributes(),
        }
    }

    fn get_attributes<'a>() -> &'a [wgpu::VertexAttribute];
}
