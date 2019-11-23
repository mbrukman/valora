//! Canvas rendering.

use crate::{canvas::*, gpu::*, Options, Result};
use glium::{glutin::EventsLoop, texture::texture2d_multisample::Texture2dMultisample, Frame};
use image::{ImageBuffer, Rgba};
use palette::{
    encoding::{srgb::Srgb, TransferFn},
    Component,
};
use rayon::prelude::*;
use std::{path::PathBuf, time::Duration};

/// The context of the current render frame.
#[derive(Debug, Copy, Clone)]
pub struct FrameContext {
    /// The current frame in the composition.
    pub frame: usize,
}

pub enum Rebuild {
    NewSeed(u64),
}

#[derive(Debug)]
struct FrameUpdates {
    new_seed: Option<u64>,
    wait: Option<Duration>,
    should_quit: bool,
}

pub enum RenderStrategy<F1, F2> {
    Screen {
        get_frame: F1,
        events_loop: EventsLoop,
        wait: Duration,
    },
    File {
        output_path: F2,
        buffer: Texture2dMultisample,
    },
}

/// A render gate renders frames.
pub struct Renderer<'a, F1, F2> {
    pub strategy: RenderStrategy<F1, F2>,
    pub gpu: &'a Gpu,
    pub options: Options,
    pub output_width: u32,
    pub output_height: u32,
}

impl<'a, F1: Fn() -> Frame + 'a, F2: Fn(usize, u64) -> PathBuf> Renderer<'a, F1, F2> {
    /// Render all of the frames for the composition. This will not return until until all frames of
    /// the composition have been rendered.
    pub fn render_frames(
        &mut self,
        mut f: impl FnMut(&FrameContext, &mut Canvas),
    ) -> Result<Option<Rebuild>> {
        let default_shader = self
            .gpu
            .default_shader(self.output_width as f32, self.output_height as f32);

        for frame in 0..(self.options.world.frames) {
            let mut canvas = Canvas::new(default_shader.clone(), self.options.world.scale);
            f(&FrameContext { frame }, &mut canvas);

            let updates = self.render_frame(self.options.world.seed, frame, canvas)?;
            if let Some(wait) = updates.wait {
                std::thread::sleep(wait);
            }

            if let Some(new_seed) = updates.new_seed {
                return Ok(Some(Rebuild::NewSeed(new_seed)));
            }
        }

        Ok(None)
    }

    fn render_frame(
        &mut self,
        current_seed: u64,
        frame_number: usize,
        canvas: Canvas,
    ) -> Result<FrameUpdates> {
        match &mut self.strategy {
            RenderStrategy::Screen {
                get_frame,
                events_loop,
                wait,
            } => {
                let mut frame = get_frame();
                frame.set_finish()?;
                self.gpu
                    .render(self.output_width, self.output_height, canvas, &mut frame)?;
                let mut should_quit = false;
                events_loop.poll_events(|event| {
                    use glutin::{DeviceEvent, Event, KeyboardInput, VirtualKeyCode};
                    match event {
                        Event::DeviceEvent {
                            event:
                                DeviceEvent::Key(KeyboardInput {
                                    virtual_keycode: Some(VirtualKeyCode::Escape),
                                    ..
                                }),
                            ..
                        } => {
                            should_quit = true;
                        }
                        _ => {}
                    }
                });

                Ok(FrameUpdates {
                    new_seed: None,
                    wait: Some(*wait),
                    should_quit,
                })
            }
            RenderStrategy::File {
                output_path,
                buffer,
            } => {
                self.gpu.render(
                    self.output_width,
                    self.output_height,
                    canvas,
                    &mut buffer.as_surface(),
                )?;
                let raw: glium::texture::RawImage2d<u8> = self.gpu.read_to_ram(&buffer)?;
                let image: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_raw(
                    self.output_width,
                    self.output_height,
                    raw.data
                        .into_par_iter()
                        .map(|v| v.convert::<f32>())
                        .map(|v: f32| <Srgb as TransferFn>::from_linear(v))
                        .map(|v| v.convert::<u8>())
                        .collect(),
                )
                .unwrap();

                image.save(output_path(frame_number, current_seed))?;

                Ok(FrameUpdates {
                    new_seed: None,
                    wait: None,
                    should_quit: false,
                })
            }
        }
    }
}