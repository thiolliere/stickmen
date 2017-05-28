use utils::UpdateContext;
use physics::update_systems::*;

pub fn add_systems(planner: &mut ::specs::Planner<UpdateContext>) {
    planner.add_system(PhysicSystem, "physic", 10);
}
