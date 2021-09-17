use bytemuck::{Pod, Zeroable};
use std::{any::type_name, marker::PhantomData, ops::RangeBounds};
use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    BufferAddress, BufferDescriptor,
};

#[derive(Debug)]
pub struct ImmutableVertexBuffer<T: Pod + Zeroable> {
    buffer: wgpu::Buffer,
    _marker: PhantomData<T>,
}

impl<T: Pod + Zeroable> ImmutableVertexBuffer<T> {
    pub fn slice<S: RangeBounds<BufferAddress>>(&self, bounds: S) -> wgpu::BufferSlice {
        self.buffer.slice(bounds)
    }
}

#[derive(Debug)]
pub struct MutableVertexBuffer<T: Pod + Zeroable> {
    cpu_buf: Vec<T>,
    gpu_buf: wgpu::Buffer,
    // capacity in number of elements
    gpu_capacity: usize,
}

impl<T: Pod + Zeroable> MutableVertexBuffer<T> {
    pub fn slice<S: RangeBounds<BufferAddress>>(&self, bounds: S) -> wgpu::BufferSlice {
        // TODO assert bounds within size?
        self.gpu_buf.slice(bounds)
    }

    pub fn size(&self) -> usize {
        self.cpu_buf.len()
    }

    pub fn write(&mut self, data: T) {
        self.cpu_buf.push(data);
    }

    pub fn reset(&mut self) {
        self.cpu_buf.clear();
    }

    pub fn update(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        if self.cpu_buf.len() > self.gpu_capacity {
            debug!(
                "Reallocating gpu vertex buffer for type: {}, to store: {} elements",
                std::any::type_name::<T>(),
                self.cpu_buf.capacity()
            );
            // TODO: use init descriptor instead if vec isn't used for cpu buf
            self.gpu_buf = device.create_buffer(&BufferDescriptor {
                label: Some(&format!("Mutable buffer of: {}", type_name::<Self>())),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                size: (self.cpu_buf.capacity() * std::mem::size_of::<T>()) as u64,
                mapped_at_creation: false,
            });
            self.gpu_capacity = self.cpu_buf.capacity();
        }
        queue.write_buffer(&self.gpu_buf, 0, bytemuck::cast_slice(&self.cpu_buf));
    }
}

/// Trait to define custom Vertex data, after STEP_MODE is defined together with descriptor
/// it's possible to easily allocate new mutable and immutable buffers  
pub trait VertexData: Pod + Zeroable {
    const STEP_MODE: wgpu::VertexStepMode;

    fn allocate_immutable_buffer(
        device: &wgpu::Device,
        buffer_data: &[Self],
    ) -> ImmutableVertexBuffer<Self> {
        ImmutableVertexBuffer {
            buffer: device.create_buffer_init(&BufferInitDescriptor {
                label: Some(&format!("Immutable buffer of: {}", type_name::<Self>())),
                usage: wgpu::BufferUsages::VERTEX,
                contents: bytemuck::cast_slice(buffer_data),
            }),
            _marker: PhantomData::default(),
        }
    }

    fn allocate_mutable_buffer(
        device: &wgpu::Device,
        buffer_data: Vec<Self>,
    ) -> MutableVertexBuffer<Self> {
        MutableVertexBuffer {
            gpu_buf: device.create_buffer_init(&BufferInitDescriptor {
                label: Some(&format!("Mutable buffer of: {}", type_name::<Self>())),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                contents: bytemuck::cast_slice(&buffer_data),
            }),
            gpu_capacity: buffer_data.len(),
            cpu_buf: buffer_data,
        }
    }

    fn allocate_mutable_buffer_with_size(
        device: &wgpu::Device,
        num_elements: usize,
    ) -> MutableVertexBuffer<Self> {
        let byte_size = std::mem::size_of::<Self>() * num_elements;
        MutableVertexBuffer {
            gpu_buf: device.create_buffer(&BufferDescriptor {
                label: Some(&format!("Mutable buffer of: {}", type_name::<Self>())),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                size: byte_size as u64,
                mapped_at_creation: false,
            }),
            gpu_capacity: num_elements,
            cpu_buf: Vec::with_capacity(num_elements),
        }
    }

    fn descriptor<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: Self::STEP_MODE,
            attributes: Self::attributes(),
        }
    }

    fn attributes<'a>() -> &'a [wgpu::VertexAttribute];
}
