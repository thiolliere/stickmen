extern crate clap;
extern crate vecmath;
extern crate unicode_normalization;
extern crate itertools;
extern crate arrayvec;
#[macro_use] extern crate glium;
#[macro_use] extern crate hlua;
#[macro_use] extern crate lazy_static;
extern crate time;
extern crate rustyline;
extern crate rusttype;
extern crate specs;
extern crate fnv;
extern crate regex;
extern crate num_cpus;

#[macro_use] extern crate ruga_utils as utils;
extern crate ruga_graphics as graphics;
extern crate ruga_physics as physics;

#[macro_use] mod entities;
mod app;
mod api;
mod resources;
mod update_systems;
mod draw_systems;
mod components;
mod colors;
mod notifications;

use glium::glutin;
use rustyline::Editor;
use rustyline::error::ReadlineError;
use regex::Regex;

use std::str::FromStr;
use std::path::Path;
use std::time::Duration;
use std::thread;
use std::sync::mpsc::{channel, TryRecvError};
use std::sync::Mutex;
use std::sync::Arc;
use std::fs::File;
use std::io::{self, Write};

pub use api::Caller;
pub use api::Callee;

const BILLION: u64 = 1_000_000_000;

fn ns_to_duration(ns: u64) -> Duration {
    let secs = ns / BILLION;
    let nanos = (ns % BILLION) as u32;
    Duration::new(secs, nanos)
}

struct MyCompleter {
    filename_completer: ::rustyline::completion::FilenameCompleter,
    function_names: Vec<String>,
}
impl MyCompleter {
    fn new(names: Vec<String>) -> MyCompleter {
        MyCompleter {
            filename_completer: ::rustyline::completion::FilenameCompleter::default(),
            function_names: names,
        }
    }
}
impl ::rustyline::completion::Completer for MyCompleter {
    fn complete(&self, line: &str, pos: usize) -> ::rustyline::Result<(usize, Vec<String>)> {
        let filename_result = self.filename_completer.complete(line, pos);
        if let Ok((0, _)) = filename_result {
            let last_word_re = Regex::new(r"(\w+)$").unwrap();
            if let Some(the_match) = last_word_re.find(line) {
                let start_re = Regex::new(&*format!(r"^{}", the_match.as_str())).unwrap();
                let mut matches = vec!();
                for name in &self.function_names {
                    if start_re.is_match(&*name) {
                        matches.push(name.clone())
                    }
                }
                Ok((0, matches))
            } else {
                Ok((0, vec!()))
            }
        } else {
            filename_result
        }
        // Ok((0,vec!()))
    }
}

fn main() {
    let matches = clap::App::new("ruga")
        .version("0.3")
        .author("thiolliere <guillaume.thiolliere@opmbx.org>")
        .about("a game in rust")
        .arg(clap::Arg::with_name("vsync")
             .short("s")
             .long("vsync")
             .help("Set vsync"))
        .arg(clap::Arg::with_name("config")
             .short("c")
             .long("config")
             .value_name("FILE")
             .help("Set configuration file (lua)")
             .validator(|s| {
                 if Path::new(&*s).exists() {
                     Ok(())
                 } else {
                     Err(format!("configuration file '{}' doesn't exist", s))
                 }
             })
             .takes_value(true))
        .arg(clap::Arg::with_name("terminal")
             .short("t")
             .long("terminal")
             .help("Set lua terminal"))
        .arg(clap::Arg::with_name("dimension")
             .short("d")
             .long("dimensions")
             .value_name("WIDTH> <HEIGHT")
             .help("Set dimensions (and unset fullscreen)")
             .validator(|s| {
                 u32::from_str(&*s)
                     .map(|_| ())
                     .map_err(|e| format!("'{}' dimension is invalid : {}", s, e))
             })
             .number_of_values(2)
             .takes_value(true))
        .arg(clap::Arg::with_name("fps")
             .short("f")
             .long("fps")
             .value_name("INT")
             .default_value("60")
             .validator(|s| {
                 u64::from_str(&*s)
                     .map(|_| ())
                     .map_err(|e| format!("'{}' fps is invalid : {}", s, e))
             })
             .help("Set multisampling")
             .takes_value(true))
        .arg(clap::Arg::with_name("multisampling")
             .short("m")
             .long("multisampling")
             .value_name("FACTOR")
             .possible_values(&["2", "4", "8", "16"])
             .help("Set multisampling")
             .takes_value(true))
        .get_matches();

    let window = {
        use glium::DisplayBuild;

        let mut builder = glutin::WindowBuilder::new().with_title("Ruga");

        if matches.is_present("vsync") {
            builder = builder.with_vsync();
        }

        builder = match matches.value_of("multisampling") {
            Some("2") => builder.with_multisampling(2),
            Some("4") => builder.with_multisampling(4),
            Some("8") => builder.with_multisampling(8),
            Some("16") => builder.with_multisampling(16),
            Some(_) => unreachable!(),
            None => builder,
        };

        builder = if let Some(mut dimensions) = matches.values_of("dimension") {
            let width = u32::from_str(dimensions.next().unwrap()).unwrap();
            let height = u32::from_str(dimensions.next().unwrap()).unwrap();
            builder.with_dimensions(width, height)
        } else {
            builder.with_fullscreen(glutin::get_primary_monitor())
        };

        builder.build_glium().unwrap()
    };
    window.get_window().unwrap().set_cursor_state(glutin::CursorState::Grab).unwrap();
    window.get_window().unwrap().set_cursor(glutin::MouseCursor::NoneCursor);

    let (api_tx, api_rx) = channel();

    let mut lua = hlua::Lua::new();
    lua.openlibs();
    api::set_lua_caller(&mut lua, api_tx.clone());
    api::set_lua_callee(&mut lua);

    if let Some(file) = matches.value_of("config") {
        lua.execute_from_reader::<(), _>(File::open(file).unwrap()).unwrap();
    }

    let lua = Arc::new(Mutex::new(lua));
    let terminal = if matches.is_present("terminal") {
        let lua_clone = lua.clone();

        let mut function_names = api::callee_function_names();
        function_names.append(&mut api::caller_function_names());

        let mut rl = Editor::<_>::new();
        rl.set_completer(Some(MyCompleter::new(function_names)));

        Some(thread::spawn(move || {
            loop {
                let readline = rl.readline("> ");
                match readline {
                    Ok(line) => {
                        use hlua::LuaError::*;

                        rl.add_history_entry(&line);
                        match lua_clone.lock().unwrap().execute::<()>(&*line) {
                            Ok(()) => (),
                            Err(SyntaxError(s)) => println!("Syntax error: {}", s),
                            Err(ExecutionError(s)) => println!("Execution error: {}", s),
                            Err(ReadError(e)) => println!("Read error: {}", e),
                            Err(WrongType) => {
                                println!("Wrong type error: lua command must return nil")
                            }
                        }
                    }
                    Err(ReadlineError::Interrupted) => {
                        lua_clone.lock().unwrap().execute::<()>("quit()").unwrap();
                        println!("^C");
                        break;
                    }
                    Err(ReadlineError::Eof) => break,
                    Err(err) => {
                        println!("Readline error: {:?}", err);
                    }
                }
            }
        }))
    } else {
        None
    };

    let mut app = app::App::new(&window);
    let fps = u64::from_str(matches.value_of("fps").unwrap()).unwrap();
    let dt_ns = BILLION / fps;
    let dt = 1.0 / fps as f32;

    // Game loop inspired by http://gameprogrammingpatterns.com/game-loop.html
    // and piston event loop
    //
    // If running out of time then slow down the game

    let mut last_time = time::precise_time_ns();
    let mut key_pressed = vec!();

    'main_loop: loop {
        // Poll events
        for event in window.poll_events() {
            use glium::glutin::Event::*;
            match event {
                Closed => break 'main_loop,
                MouseInput(state, button) => {
                    use glium::glutin::MouseButton::*;

                    let state = format!("\"{:?}\"", state).to_lowercase();
                    let code: u32 = match button {
                        Left => 0 + 1 << 8,
                        Right => 1 + 1 << 8,
                        Middle => 2 + 1 << 8,
                        Other(c) => c as u32 + 1 << 9,
                    };
                    let virtualcode = match button {
                        Left | Right | Middle => format!("\"mouse{:?}\"", button).to_lowercase(),
                        Other(c) => format!("\"mouse{:x}\"", c),
                    };
                    let command = format!("input({},{},{})", state, code, virtualcode);
                    lua.lock().unwrap().execute::<()>(&*command).unwrap();
                }
                MouseMoved(x, y) => {
                    let (w, h) = window.get_window().unwrap().get_inner_size_pixels().unwrap();

                    let x = (2 * x - w as i32) as f32 / w as f32;
                    let y = -(2 * y - h as i32) as f32 / w as f32;

                    app.set_cursor(x, y);

                    let command = format!("mouse_moved({},{})", x, y);
                    lua.lock().unwrap().execute::<()>(&*command).unwrap();
                }
                KeyboardInput(state, code, virtualcode) => {
                    use glium::glutin::ElementState::*;

                    match state {
                        Pressed => if key_pressed.contains(&code) {
                            continue
                        } else {
                            key_pressed.push(code)
                        },
                        Released => key_pressed.retain(move |&v| {
                            v != code
                        }),
                    }

                    let state = format!("\"{:?}\"", state).to_lowercase();
                    let virtualcode = match virtualcode {
                        Some(c) => format!("\"{:?}\"", c).to_lowercase(),
                        None => "\"none\"".into(),
                    };
                    let command = format!("input({},{},{})", state, code, virtualcode);
                    lua.lock().unwrap().execute::<()>(&*command).unwrap();
                }
                MouseWheel(delta, _) => {
                    use glium::glutin::MouseScrollDelta::*;

                    let (h, v) = match delta {
                        LineDelta(h, v) => (h, v),
                        PixelDelta(h, v) => (h, v),
                    };
                    let command = format!("mouse_wheel({},{})", h, v);
                    lua.lock().unwrap().execute::<()>(&*command).unwrap();
                }
                Refresh => app.draw(window.draw()),
                Resized(w, h) => app.resized(w, h),
                _ => (),
            }
        }
        {
            lua.lock()
                .unwrap()
                .execute::<()>(&*format!("update({})", dt))
                .unwrap();
        }
        loop {
            match api_rx.try_recv() {
                Ok(msg) => app.call(msg),
                Err(TryRecvError::Disconnected) => break,
                Err(TryRecvError::Empty) => break,
            }
        }
        if app.must_quit() {
            break 'main_loop;
        }

        // Update
        app.update(dt);

        // Draw
        app.draw(window.draw());

        let elapsed = time::precise_time_ns() - last_time;
        if elapsed < dt_ns {
            last_time = last_time + dt_ns;
            thread::sleep(ns_to_duration(dt_ns - elapsed));
        } else {
            last_time = time::precise_time_ns();
        }
    }

    if let Some(terminal) = terminal {
        // TODO Draw explicit message
        // window.draw().finish().unwrap();
        // TODO Coloring print
        print!("[window has closed]");
        io::stdout().flush().unwrap();
        terminal.join().unwrap();
        print!("\n");
    }
}
