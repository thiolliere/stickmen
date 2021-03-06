use std::collections::HashSet;

use specs::Join;

use super::*;
use super::components::*;

impl_resource! {
    PhysicWorld,
}

pub struct PhysicWorld {
    pub inert: ::fnv::FnvHashMap<[i32; 2], Vec<EntityInformation>>,
    pub movable: ::fnv::FnvHashMap<[i32; 2], Vec<EntityInformation>>,
}
impl PhysicWorld {
    pub fn new() -> Self {
        PhysicWorld {
            inert: ::fnv::FnvHashMap::default(),
            movable: ::fnv::FnvHashMap::default(),
        }
    }
    pub fn fill(&mut self, world: &::specs::World) {
        let dynamics = world.read::<PhysicDynamic>();
        let statics = world.read::<PhysicStatic>();
        let states = world.read::<PhysicState>();
        let types = world.read::<PhysicType>();
        let entities = world.entities();

        self.inert.clear();
        self.movable.clear();

        for (_, state, typ, entity) in (&dynamics, &states, &types, &entities).iter() {
            let info = EntityInformation {
                entity: entity,
                pos: state.pos,
                group: typ.group,
                mask: typ.mask,
                shape: typ.shape.clone(),
            };
            self.insert_dynamic(info);
        }
        for (_, state, typ, entity) in (&statics, &states, &types, &entities).iter() {
            let info = EntityInformation {
                entity: entity,
                pos: state.pos,
                group: typ.group,
                mask: typ.mask,
                shape: typ.shape.clone(),
            };
            self.insert_static(info);
        }
    }
    pub fn insert_dynamic(&mut self, info: EntityInformation) {
        for cell in info.shape.cells(info.pos) {
            self.movable.entry(cell).or_insert(Vec::new()).push(info.clone());
        }
    }
    pub fn insert_static(&mut self, info: EntityInformation) {
        for cell in info.shape.cells(info.pos) {
            self.inert.entry(cell).or_insert(Vec::new()).push(info.clone());
        }
    }
    /// The collision is between shape and other entity
    pub fn apply_on_shape<F: FnMut(&EntityInformation, &Collision)>(&self, shape: &ShapeCast, callback: &mut F) {
        let null_vec = Vec::new();
        for cell in shape.shape.cells(shape.pos) {
            let inert = self.inert.get(&cell).unwrap_or(&null_vec).iter();
            let movable = self.movable.get(&cell).unwrap_or(&null_vec).iter();

            for entity in inert.chain(movable) {
                if shape.not.contains(&entity.entity) { continue; }
                if entity.group & shape.mask == 0 { continue; }
                if entity.mask & shape.group == 0 { continue; }
                if let Some(collision) = shape_collision(shape.pos, &shape.shape, entity.pos, &entity.shape) {
                    callback(entity, &collision);
                }
            }
        }
    }
    pub fn raycast<F: FnMut((&EntityInformation, f32, f32)) -> ContinueOrStop>(&self, ray: &RayCast, callback: &mut F) {
        use ::std::f32::consts::FRAC_PI_4;
        use ::std::f32::consts::PI;
        use ::std::cmp::Ordering;

        enum Direction {
            Left,
            Right,
            Up,
            Down,
        }
        impl Direction {
            fn signum(&self, x0: f32, y0: f32, x1: f32, y1: f32) -> f32 {
                match *self {
                    Direction::Left => (x1 - x0).signum(),
                    Direction::Right => (x0 - x1).signum(),
                    Direction::Up => (y0 - y1).signum(),
                    Direction::Down => (y1 - y0).signum(),
                }
            }
        }
        let angle = ::utils::minus_pi_pi(ray.angle);
        let direction = if angle.abs() > 3. * FRAC_PI_4 {
            Direction::Left
        } else if angle.abs() < FRAC_PI_4 {
            Direction::Right
        } else if angle > 0. {
            Direction::Up
        } else {
            Direction::Down
        };

        let ox = ray.origin[0];
        let oy = ray.origin[1];
        let dx = ox + ray.length * angle.cos();
        let dy = oy + ray.length * angle.sin();
        let cells = grid_raycast(ox, oy, dx, dy);

        // equation ax + by + c = 0
        let equation = if angle.abs() == PI || angle == 0. {
            (0., 1., -oy)
        } else {
            let b = -1. / angle.tan();
            (1., b, -ox - b * oy)
        };

        let null_vec = Vec::new();
        let mut visited = HashSet::new();
        let mut entities = Vec::new();

        for cell in cells {
            let current_length = ray.length;

            let possible_entities = self.movable
                .get(&cell)
                .unwrap_or(&null_vec)
                .iter()
                .chain(self.inert.get(&cell).unwrap_or(&null_vec).iter());

            for entity in possible_entities {
                if ray.not.contains(&entity.entity) {
                    continue;
                }
                if entity.group & ray.mask == 0 {
                    continue;
                }
                if entity.mask & ray.group == 0 {
                    continue;
                }
                if visited.contains(&entity.entity) {
                    continue;
                }
                visited.insert(entity.entity);

                if let Some((x0, y0, x1, y1)) = entity.shape.raycast(entity.pos, equation) {
                    let l1 = ((ox - x0).powi(2) + (oy - y0).powi(2)).sqrt() *
                        direction.signum(x0, y0, ox, oy);
                    let l2 = ((ox - x1).powi(2) + (oy - y1).powi(2)).sqrt() *
                        direction.signum(x1, y1, ox, oy);

                    let min = l1.min(l2);
                    let max = l1.max(l2);

                    if max < 0. || min > ray.length {
                        continue;
                    }

                    entities.push((entity, min, max));
                }
            }

            let mut called = vec![];
            let mut i = 0;
            while i < entities.len() {
                let (_, min, _) = entities[i];
                if min <= current_length {
                    called.push(entities.swap_remove(i))
                } else {
                    i += 1;
                }
            }

            called.sort_by(|&(_, min_a, _), &(_, min_b, _)| {
                if min_a > min_b {
                    Ordering::Greater
                } else if min_a == min_b {
                    Ordering::Equal
                } else {
                    Ordering::Less
                }
            });

            for entity in called {
                if let ContinueOrStop::Stop = callback(entity) {
                    return;
                }
            }
        }
    }
}
