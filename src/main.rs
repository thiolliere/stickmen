#[macro_use] extern crate configuration;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate nom;
extern crate baal;
extern crate graphics;
extern crate glium;
extern crate hlua;
extern crate specs;
extern crate time;
extern crate toml;
extern crate rand;

mod levels;
mod app;
mod conf;
pub mod doors;
pub mod signal_network;
pub mod effects;
pub mod event_loop;
pub mod weapons;
pub mod control;
pub mod physic;
pub mod entities;
pub mod utils;

pub use conf::{config,snd_effect,music};
pub use utils::Direction;
pub use utils::key;
use glium::glutin::ElementState;
use glium::glutin::Event as InputEvent;
use std::time::Duration;
use std::thread;
use event_loop::{
    Events,
    Event,
};

fn init() -> Result<(app::App,glium::backend::glutin_backend::GlutinFacade,event_loop::WindowEvents),String> {
    use glium::DisplayBuild;

    // init baal
    try!(baal::init(&baal::Setting {
        channels: config.audio.channels,
        sample_rate: config.audio.sample_rate,
        frames_per_buffer: config.audio.frames_per_buffer,
        effect_dir: config.audio.effect_dir.clone(),
        music_dir: config.audio.music_dir.clone(),
        global_volume: config.audio.global_volume,
        music_volume: config.audio.music_volume,
        effect_volume: config.audio.effect_volume,
        distance_model: match &*config.audio.distance_model {
            "linear" => baal::effect::DistanceModel::Linear(config.audio.distance_model_min,config.audio.distance_model_max),
            "pow2" => baal::effect::DistanceModel::Pow2(config.audio.distance_model_min,config.audio.distance_model_max),
            _ => unreachable!(),
        },
        music_loop: config.audio.music_loop,
        effect: config.audio.effect.to_vec(),
        music: config.audio.music.to_vec(),
        check_level: match &*config.audio.check_level {
            "never" => baal::CheckLevel::Never,
            "always" => baal::CheckLevel::Always,
            "debug" => baal::CheckLevel::Debug,
            _ => unreachable!(),
        },
    }).map_err(|e| format!("ERROR: audio init failed: {:#?}",e)));

    // init window
    // TODO if fail then disable vsync and then multisampling and then vsync and multisamping
    let window = {
        let mut builder = glium::glutin::WindowBuilder::new()
            .with_dimensions(config.window.dimension[0], config.window.dimension[1])
            .with_title(format!("ruga"));

        if config.window.vsync {
            builder = builder.with_vsync();
        }
        if config.window.multisampling != 0 {
            builder = builder.with_multisampling(config.window.multisampling)
        }
        try!(builder.build_glium().map_err(|e| format!("ERROR: window init failed: {}",e)))
    };
    window.get_window().unwrap().set_cursor_state(glium::glutin::CursorState::Hide).unwrap();

    // init app
    let app = try!(app::App::new(&window));

    // init event loop
    let window_events = window.events(&event_loop::Setting {
        ups: config.event_loop.ups,
        max_fps: config.event_loop.max_fps,
    });


    Ok((app,window,window_events))
}

fn main() {
    // init
    let (mut app,mut window,mut window_events) = match init() {
        Ok(app) => app,
        Err(err) => {
            println!("{}",err);
            std::process::exit(1);
        },
    };

    // game loop
    while let Some(event) = window_events.next(&mut window) {
        match event {
            Event::Update(args) => app.update(args),
            Event::Render(args) => app.render(args),
            Event::Input(InputEvent::Closed) => break,
            Event::Input(InputEvent::KeyboardInput(state,keycode,_)) => {
                if state == ElementState::Pressed {
                    app.key_pressed(keycode);
                } else {
                    app.key_released(keycode);
                }
            },
            Event::Input(InputEvent::MouseInput(state,button)) => {
                if state == ElementState::Pressed {
                    app.mouse_pressed(button);
                } else {
                    app.mouse_released(button);
                }
            },
            Event::Input(InputEvent::MouseMoved(x,y)) => {
                println!("mouse move {:?} {:?}",x,y);
                let dimension = window.get_framebuffer_dimensions();

                let dimension = [dimension.0 as f32, dimension.1 as f32];
                let x = x as f32;
                let y = y as f32;

                app.mouse_moved((x-dimension[0]/2.)/dimension[0]*2., (-y+dimension[1]/2.)/dimension[1]*2.);
            },
            Event::Input(InputEvent::Resized(width,height)) => {
                app.resize(width,height);
            },
            Event::Input(_) => (),
            Event::Idle(args) => thread::sleep(Duration::from_millis(args.dt as u64)),
        }
    }
}

