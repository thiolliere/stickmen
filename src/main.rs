#[macro_use] extern crate configuration;
#[macro_use] extern crate lazy_static;
extern crate baal;
extern crate graphics;
extern crate glium;
extern crate specs;
extern crate time;
extern crate toml;
extern crate rand;
extern crate bmp;

mod levels;
mod app;
mod conf;
mod event_loop;
mod control;
mod physic;
mod entities;
mod utils;
mod life;
mod portal;

mod components {
    pub use control::{
        PlayerControl,
        TowardPlayerControl,
        MonsterControl,
    };
    pub use physic::{
        GridSquare,
        PhysicState,
        PhysicType,
        PhysicForce,
        PhysicWorld,
        PhysicDynamic,
        PhysicStatic,
        PhysicTrigger,
        Shape,
        Ray,
        CollisionBehavior,
    };
    pub use life::{
        Column,
        Life,
        Killer,
        Ball,
    };
    pub use portal::Portal;
    pub use app::Graphic;
}
mod systems {
    pub use physic::PhysicSystem;
    pub use life::{
        LifeSystem,
        KillerSystem,
        BallSystem,
        ColumnSystem,
    };
    pub use control::{
        PlayerSystem,
        MonsterSystem,
        TowardPlayerSystem,
    };
    pub use portal::PortalSystem;
}

pub use conf::{config};

use glium::glutin::ElementState;
use glium::glutin::Event as InputEvent;
use glium::glutin;
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
        effect: {
            let mut effect = vec!();
            for i in 0..config.audio.effects_name.len() {
                effect.push((config.audio.effects_name[i].clone(),config.audio.effects_number[i].clone()));
            }
            effect
        },
        music: config.audio.musics.clone(),
        check_level: match &*config.audio.check_level {
            "never" => baal::CheckLevel::Never,
            "always" => baal::CheckLevel::Always,
            "debug" => baal::CheckLevel::Debug,
            _ => unreachable!(),
        },
        music_transition: match &*config.audio.transition_type {
            "instant" => baal::music::MusicTransition::Instant,
            "smooth" => baal::music::MusicTransition::Smooth(config.audio.transition_time),
            "overlap" => baal::music::MusicTransition::Overlap(config.audio.transition_time),
            _ => unreachable!(),
        },
    }).map_err(|e| format!("ERROR: audio init failed: {:#?}",e)));

    // init window
    // TODO if fail then disable vsync and then multisampling and then vsync and multisamping
    let window = {
        let mut builder = glium::glutin::WindowBuilder::new();

        if config.window.vsync {
            builder = builder.with_vsync();
        }
        if config.window.multisampling != 0 {
            builder = builder.with_multisampling(config.window.multisampling);
        }
        if config.window.fullscreen {
            if config.window.fullscreen_on_primary_monitor {
                builder = builder.with_fullscreen(glutin::get_primary_monitor());
            } else {
                builder = builder.with_fullscreen(try!(glutin::get_available_monitors().nth(config.window.fullscreen_monitor)
                                                  .ok_or("ERROR: window init failed: fullsceen monitor specified unavailable")));
            }
        } else {
            builder = builder.with_dimensions(config.window.dimension[0], config.window.dimension[1])
                .with_title(format!("ruga"));
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
            Event::Input(InputEvent::Resized(width,height)) => {
                app.resize(width,height);
            },
            Event::Input(_) => (),
            Event::Idle(args) => thread::sleep(Duration::from_millis(args.dt as u64)),
        }

        if app.quit {
            baal::close();
            return;
        }
    }
}

#[test]
fn main_test() {
    if let Err(err) = init() {
        println!("{}",err);
        std::process::exit(1);
    }
}
