use legion::{Resources, Schedule, World};
use winit::{dpi::PhysicalSize, event::WindowEvent, window::Window};

use crate::graphics::renderer::Renderer;

pub struct App {
    renderer: Renderer,
    world: World,
    resources: Resources,
    schedule: Schedule,
    // move to renderer?
    pub size: PhysicalSize<u32>, 
}

impl App {
    pub async fn new(window: &Window) -> App {
        let size = window.inner_size();
        let renderer = Renderer::new(window).await;

        let world = World::default();
        let resources = Resources::default();
        let schedule = Schedule::builder().build();
        App {
            size,
            world,
            schedule,
            resources,
            renderer,
        }
    }

    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        self.renderer.resize(new_size);
    }

    pub fn did_handle_input(&mut self, _event: &WindowEvent) -> bool {
        false
    }

    pub fn update(&mut self) {
        self.schedule.execute(&mut self.world, &mut self.resources);
        self.renderer.update();
    }

    pub fn render(&mut self) -> Result<(), wgpu::SwapChainError> {
        self.renderer.render()?;
        Ok(())
    }
}
