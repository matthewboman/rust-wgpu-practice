use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder}
};

#[cfg(target_arch="wasm32")]
use wasm_bindgen::prelude::*;

struct State {
    surface:         wgpu::Surface,
    device:          wgpu::Device,
    queue:           wgpu::Queue,
    config:          wgpu::SurfaceConfiguration,
    size:            winit::dpi::PhysicalSize<u32>,
    window:          Window,
    render_pipeline: wgpu::RenderPipeline,
}

impl State {
    // Creating some of the wgpu types requires async code
    async fn new(window: Window) -> Self {
        let size = window.inner_size();

        // The instance is a handle to our GPU
        let instance = wgpu::Instance::new(wgpu::Backends::all());

        // # Safety
        //
        // The surface needs to live as long as the window that created it.
        // State owns the window so this should be safe.
        let surface = unsafe { instance.create_surface(&window) };
        let adapter = instance.request_adapter(
            &wgpu::RequestAdapterOptions {
                power_preference:       wgpu::PowerPreference::default(),
                compatible_surface:     Some(&surface),
                force_fallback_adapter: false,
            }
        ).await.unwrap();

        /*
         * Enumerator to fall back on if `adapter` returns `None`

         let adapter = instance.enumerate_adapters(wgpu::Backends::all())
            .filter(|adapter| {
                !surface.get_supported_formats(&adapter).is_empty()
            })
            .next()
            .unwrap();
         */

         let (device, queue) = adapter.request_device(
             &wgpu::DeviceDescriptor {
                 features: wgpu::Features::empty(),
                 // WebGL doesn't support all wgpu's features, so disable some if building for web.
                 limits:   if cfg!(target_arch = "wasm32") {
                     wgpu::Limits::downlevel_webgl2_defaults()
                 } else {
                     wgpu::Limits::default()
                 },
                 label:    None,
             },
             None, // trace path
         ).await.unwrap();

         let config = wgpu::SurfaceConfiguration {
             usage:        wgpu::TextureUsages::RENDER_ATTACHMENT,
             format:       surface.get_supported_formats(&adapter)[0], // the prefered format is placed at the beginning of the vector
             width:        size.width,
             height:       size.height,
             present_mode: wgpu::PresentMode::Fifo, // VSync, likely supported on all platforms
             alpha_mode:   wgpu::CompositeAlphaMode::Auto,
         };

         let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
             label:  Some("Shader"),
             source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
         });

         let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts:   &[],
            push_constant_ranges: &[],
         });

         let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label:    Some("Render Pipeline"),
            layout:   Some(&render_pipeline_layout),
            vertex:   wgpu::VertexState {
                module:      &shader,
                entry_point: "vs_main",
                buffers:     &[],
            },
            fragment: Some(wgpu::FragmentState {
                module:      &shader,
                entry_point: "fs_main",
                targets:     &[Some(wgpu::ColorTargetState {
                    format:     config.format,
                    blend:      Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology:           wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face:         wgpu::FrontFace::Ccw,
                cull_mode:          Some(wgpu::Face::Back),
                polygon_mode:       wgpu::PolygonMode::Fill,
                unclipped_depth:    false,
                conservative:       false,
            },
            depth_stencil: None,
            multisample:   wgpu::MultisampleState {
                count: 1,
                mask:  !0,
                alpha_to_coverage_enabled: false
            },
            multiview: None,
         });

         surface.configure(&device, &config);

         Self {
             window,
             surface,
             device,
             queue,
             config,
             size,
             render_pipeline,
         }
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size          = new_size;
            self.config.width  = new_size.width;
            self.config.height = new_size.height;

            self.surface.configure(&self.device, &self.config);
        }
    }

    fn input(&mut self, event: &WindowEvent) -> bool {
        // return FALSE bc we don't have any events we want to capture
        false
    }

    fn update(&mut self) {

    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output      = self.surface.get_current_texture()?;
        let view        = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        // `begin_render_pass()` borrows `encoder`. We want to drop any variables when
        // it leaves scope, thus releasing `encoder` so we can call `.finish()`
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops:  wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.2,
                            b: 0.3,
                            a: 1.0,
                        }),
                        store: true
                    },
                })],
                depth_stencil_attachment: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.draw(0..3, 0..1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub async fn run() {
    // Toggle logger based on WASM or desktop
    cfg_if::cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            std::panic::set_hook(Box::new(console_error_panic_hook::hook));
            console_log::init_with_level(log::Level::Warn).expect("Couldn't initialize logger");
        } else {
            env_logger::init();
        }
    }

    // Window setup
    let event_loop = EventLoop::new();
    let window     = WindowBuilder::new().build(&event_loop).unwrap();

    // Add a canvas to the HTML document
    #[cfg(target_arch = "wasm32")]
    {
        use winit::dpi::PhysicalSize;
        use winit::platform::web::WindowExtWebSys;

        window.set_inner_size(PhysicalSize::new(450, 400));

        web_sys::window()
            .and_then(|win| win.document())
            .and_then(|doc| {
                let dst    = doc.get_element_by_id("wasm-example")?;
                let canvas = websys::Element::from(window.canvas());

                dst.append_child(&canvas).ok()?;

                Some(())
            })
            .expect("Couldn't append canvas to document body");
    }

    // State::new uses async code, so wait to finish
    let mut state = State::new(window).await;

    // Event loop
    event_loop.run(move |event, _, control_flow| match event {
        Event::WindowEvent {
            ref event,
            window_id,
        } if window_id == state.window.id() => {
            if !state.input(event) {
                match event {
                    WindowEvent::CloseRequested | WindowEvent::KeyboardInput {
                        input: KeyboardInput {
                            state: ElementState::Pressed,
                            virtual_keycode: Some(VirtualKeyCode::Escape),
                            ..
                        },
                        ..
                    } => *control_flow = ControlFlow::Exit,
                    WindowEvent::Resized(physical_size) => {
                        state.resize(*physical_size);
                    }
                    WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                        state.resize(**new_inner_size) // dereference it bc it's &&mut
                    }
                    _ => {}
                }
            }
        }
        Event::RedrawRequested(window_id) if window_id == state.window().id() => {
            state.update();
            match state.render() {
                Ok(_) => {},
                // Reconfigure the surface if lost
                Err(wgpu::SurfaceError::Lost) => state.resize(state.size),
                // The system is out of memory--quit
                Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                // All other errors (Outdated, Timeout) should be resolved by the next frame
                Err(e) => eprintln!("{:?}", e),
            }
        }
        Event::MainEventsCleared => {
            // RedrawRequested will only trigger once unless we manually retrigger it
            state.window().request_redraw();
        }
        _ => {}

    });

}
