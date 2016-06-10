use specs;
use entities;
use hlua;
use hlua::Lua;
// use hlua::any::AnyLuaValue;
use std::fs::File;
use config;
use std::path::Path;
use std::io;
use specs::Join;
use physic;

#[derive(Debug)]
pub enum LoadError {
    OpenFile(io::Error),
    Lua(hlua::LuaError),
}

pub fn load<'l>(level: String, world: &specs::World) -> Result<specs::Entity,LoadError>{
    // flush world
    for entity in world.entities().iter() {
        world.delete_later(entity);
    }
    world.maintain();

    let path = Path::new(&*config.levels.dir).join(Path::new(&*format!("{}{}",level,".lua")));

    let file = try!(File::open(&path).map_err(|e| LoadError::OpenFile(e)));

    let mut lua: Lua<'l> = Lua::new();

    //lua.set("add_character", hlua::function2(|x: f32,y: f32| {
    //    entities.character.build(world,[x,y]);
    //}));
    //lua.set("add_monster", hlua::function2(|x: f32,y: f32| {
    //    entities.monster.build(world,[x,y]);
    //}));

    try!(lua.execute_from_reader::<(),_>(file).map_err(|e| LoadError::Lua(e)));

    // init world
    let master_entity = world.create_now()
        .with::<physic::PhysicWorld>(physic::PhysicWorld::new())
        .build();
    let mut physic_worlds = world.write::<physic::PhysicWorld>();
    physic_worlds.get_mut(master_entity).unwrap().fill(&world);

    Ok(master_entity)
}

