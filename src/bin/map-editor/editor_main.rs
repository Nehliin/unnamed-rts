#[macro_use]
extern crate log;

use edit_state::EditState;
use futures::executor::block_on;
use mimalloc::MiMalloc;
use unnamed_rts::{engine::Engine, states::State};
use winit::{
    event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

mod edit_state;
mod editor_systems;
mod playground_state;
mod playground_systems;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .init();
    let event_loop = EventLoop::new();

    let window = WindowBuilder::new()
        .with_title("RTS!")
        .build(&event_loop)
        .expect("Failed to create window");
    let mut app = block_on(Engine::new(&window));
    app.push_state(Box::new(EditState::default()) as Box<dyn State>);
    event_loop.run(move |event, _, control_flow| {
        if !app.event_handler(&event) {
            match event {
                Event::WindowEvent {
                    ref event,
                    window_id,
                } if window_id == window.id() => match event {
                    WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                    WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                state: ElementState::Pressed,
                                virtual_keycode: Some(VirtualKeyCode::Escape),
                                ..
                            },
                        ..
                    } => *control_flow = ControlFlow::Exit,
                    _ => {}
                },
                Event::RedrawRequested(_) => {
                    match app.render() {
                        Ok(_) => {}
                        // Recreate the swap_chain if lost
                        Err(wgpu::SurfaceError::Lost) => app.recreate_swap_chain(),
                        // The system is out of memory, we should probably quit
                        Err(wgpu::SurfaceError::OutOfMemory) => {
                            error!("SwapChain out of memory!");
                            *control_flow = ControlFlow::Exit
                        }
                        // All other errors (Outdated, Timeout) should be resolved by the next frame
                        Err(e) => debug!("{:?}", e),
                    }
                    *control_flow = ControlFlow::Poll;
                }
                Event::MainEventsCleared => {
                    // RedrawRequested will only trigger once, unless we manually
                    // request it.
                    window.request_redraw();
                }
                _ => {}
            }
        }
    });
}
