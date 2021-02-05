use application::App;
use futures::executor::block_on;
use log::warn;
use winit::{
    event::{ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
mod input;
mod application;
mod assets;
mod components;
mod graphics;

fn main() {
    env_logger::init();
    let event_loop = EventLoop::new();

    let window = WindowBuilder::new()
        .with_title("RTS!")
        .build(&event_loop)
        .expect("Failed to create window");
    let mut app = block_on(App::new(&window));
    event_loop.run(move |event, _, control_flow| {
        app.event_handler(&event, &window.id());
        match event {
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == window.id() => {
                match event {
                    WindowEvent::CloseRequested => *control_flow = ControlFlow::Exit,
                    WindowEvent::Resized(physical_size) => {
                        //app.resize(*physical_size, None);
                    }
                    WindowEvent::ScaleFactorChanged {
                        new_inner_size,
                        scale_factor,
                    } => {
                       //  new_inner_size is &&mut so we have to dereference it twice
                        //app.resize(**new_inner_size, Some(*scale_factor as f32));
                    }
                    WindowEvent::KeyboardInput { input, .. } => {
                        if let KeyboardInput {
                            state: ElementState::Pressed,
                            virtual_keycode: Some(VirtualKeyCode::Escape),
                            ..
                        } = input
                        {
                            *control_flow = ControlFlow::Exit
                        }
                    }
                    _ => {}
                }
            }
            Event::RedrawRequested(_) => {
                match app.render() {
                    Ok(_) => {}
                    // Recreate the swap_chain if lost
                    Err(wgpu::SwapChainError::Lost) => app.recreate_swap_chain(),
                    // The system is out of memory, we should probably quit
                    Err(wgpu::SwapChainError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                    // All other errors (Outdated, Timeout) should be resolved by the next frame
                    Err(e) => warn!("{:?}", e),
                }
            }
            Event::MainEventsCleared => {
                // RedrawRequested will only trigger once, unless we manually
                // request it.
                window.request_redraw();
            }
            _ => {}
        }
    });
}
