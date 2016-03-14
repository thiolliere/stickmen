use rand;
use rand::distributions::{IndependentSample, Range};
use world::spatial_hashing::Location;
use world::batch::Batch;
use world::World;
use super::{
    Body,
    BodyTrait,
    BodyType,
    CollisionBehavior,
    PhysicType,
    group,
};
use std::cell::RefCell;
use std::f64;
use util::minus_pi_pi;
use frame_manager::{color, FrameManager};
use effect_manager::{EffectManager, Line, Effect};

pub const SWORD_RECOVER: f64 = 0.8;
pub const SWORD_LENGTH: f64 = 5.;
pub const SWORD_DAMAGE: f64 = 5.;

struct Sword {
    recover: f64,
}

impl Sword {
    fn new() -> Sword {
        Sword {
            recover: 0.,
        }
    }

    fn update(&mut self, dt: f64) {
        if self.recover > 0. {
            self.recover = (self.recover - dt).max(0.);
        }
    }
}

trait SwordManager {
    fn sword_attack(&self, batch: &Batch, effect_manager: &mut EffectManager);
}

impl SwordManager for RefCell<Character> {
    fn sword_attack(&self,batch: &Batch,effect_manager: &mut EffectManager) {
        use std::f64::consts::{PI, FRAC_PI_2};

        if self.borrow().sword.recover <= 0. {
            self.borrow_mut().sword.recover = SWORD_RECOVER;

            let (id,x,y,aim) = {
                let this = self.borrow();
                (this.id(),this.x(), this.y(), minus_pi_pi(self.aim()))
            };
            let loc = Location {
                up: y + SWORD_LENGTH,
                down: y - SWORD_LENGTH,
                left: x - SWORD_LENGTH,
                right: x + SWORD_LENGTH,
            };


            batch.apply_locally(!group::ARMORY, &loc, &mut |body: &mut BodyTrait| {
                if body.id() != id && body.in_circle([x,y],SWORD_LENGTH) {
                    let in_part = if aim == PI {
                        body.left() <= x
                    } else if aim > FRAC_PI_2 {
                        let t_x = body.left() - x;
                        let t_y = body.up() - y;
                        let a = aim - FRAC_PI_2;
                        t_y >= a * t_x
                    } else if aim == FRAC_PI_2 {
                        body.up() >= y
                    } else if aim > 0. {
                        let t_x = body.right() - x;
                        let t_y = body.up() - y;
                        let a = aim - FRAC_PI_2;
                        t_y >= a * t_x
                    } else  if aim == 0. {
                        body.right() >= x
                    } else if aim > -FRAC_PI_2 {
                        let t_x = body.right() - x;
                        let t_y = body.down() - y;
                        let a = aim - FRAC_PI_2;
                        t_y >= a * t_x
                    } else if aim == -FRAC_PI_2 {
                        body.down() <= y
                    } else {
                        let t_x = body.left() - x;
                        let t_y = body.down() - y;
                        let a = aim - FRAC_PI_2;
                        t_y >= a * t_x
                    };

                    if in_part {
                        body.damage(SWORD_DAMAGE);
                    }
                }
            });

            effect_manager.add(Effect::SwordAttack(Line::new(x,y,aim,SWORD_LENGTH)));
        }
    }
}

#[derive(Clone,Copy,PartialEq)]
pub enum GunType {
    None,
    Rifle,
    Shotgun,
    Sniper,
}

struct Gun {
    gun_type: GunType,
    next_type: GunType,
    reloading: f64,
    shooting: bool,
    ammo: u32,
}

pub const RIFLE_RELOADING_TIME: f64 = 0.1;
pub const SHOTGUN_RELOADING_TIME: f64 = 0.8;
pub const SNIPER_RELOADING_TIME: f64 = 1.5;

pub const RIFLE_LENGTH: f64 = 30.;
pub const SHOTGUN_LENGTH: f64 = 30.;
pub const SNIPER_LENGTH: f64 = 70.;

pub const RIFLE_DAMAGE: f64 = 10.;
pub const SHOTGUN_DAMAGE: f64 = 10.;
pub const SNIPER_DAMAGE: f64 = 100.;

pub const RIFLE_MAX_DELTA_ANGLE: f64 = f64::consts::PI/16.;
pub const SHOTGUN_MAX_DELTA_ANGLE: f64 = f64::consts::PI/6.;
pub const SHOTGUN_SHOOT_NUMBER: u32 = 4;

impl Gun {
    pub fn new() -> Gun {
        Gun {
            gun_type: GunType::Rifle,
            next_type: GunType::Rifle,
            shooting: false,
            reloading: 0.,
            ammo: 10000000,
        }
    }

    pub fn time_to_reload(&mut self) -> f64 {
        match self.gun_type {
            GunType::Sniper => SNIPER_RELOADING_TIME,
            GunType::Shotgun => SHOTGUN_RELOADING_TIME,
            GunType::Rifle => RIFLE_RELOADING_TIME,
            GunType::None => 0.,
        }
    }
}

trait GunManager {
    fn gun_shoot(&self, batch: &Batch, effect_manager: &mut EffectManager);
    fn gun_update(&self, dt: f64, batch: &Batch, effect_manager: &mut EffectManager);
}

impl GunManager for RefCell<Character> {
    fn gun_shoot(&self,batch: &Batch, effect_manager: &mut EffectManager) {
        let (id,x,y,aim,gun_type) = {
            let this = self.borrow();
            (this.body.id,this.body.x,this.body.y,this.aim,this.gun.gun_type)
        };
        match gun_type {
            GunType::Sniper => {
                let mut length = SNIPER_LENGTH;
                batch.raycast(!group::ARMORY,x,y,aim,SNIPER_LENGTH, &mut |body,min,_| {
                    if body.id() != id {
                        match body.body_type() {
                            BodyType::Wall | BodyType::MovingWall => {
                                length = min;
                                true
                            },
                            _ => {
                                body.damage(SNIPER_DAMAGE);
                                false
                            }
                        }
                    } else {
                        false
                    }
                });
                effect_manager.add(Effect::SniperShoot(Line::new(x,y,aim,length)));
            },
            GunType::Shotgun => {
                let range = Range::new(-SHOTGUN_MAX_DELTA_ANGLE,SHOTGUN_MAX_DELTA_ANGLE);
                let mut rng = rand::thread_rng();
                let mut shoots = vec!();
                for _ in 0..SHOTGUN_SHOOT_NUMBER {
                    let aim = aim + range.ind_sample(&mut rng);
                    let mut length = SHOTGUN_LENGTH;
                    batch.raycast(!group::ARMORY,x,y,aim,SHOTGUN_LENGTH, &mut |body,min,_| {
                        if body.id() != id {
                            body.damage(SHOTGUN_DAMAGE);
                            length = min;
                            true
                        } else {
                            false
                        }
                    });
                    shoots.push(Line::new(x,y,aim,length));
                }
                effect_manager.add(Effect::ShotgunShoot(shoots));
            },
            GunType::Rifle => {
                let range = Range::new(-RIFLE_MAX_DELTA_ANGLE,RIFLE_MAX_DELTA_ANGLE);
                let mut rng = rand::thread_rng();
                let aim = aim + range.ind_sample(&mut rng);
                let mut length = RIFLE_LENGTH;
                batch.raycast(!group::ARMORY,x,y,aim,RIFLE_LENGTH, &mut |body,min,_| {
                    if body.id() != id {
                        body.damage(RIFLE_DAMAGE);
                        length = min;
                        true
                    } else {
                        false
                    }
                });
                effect_manager.add(Effect::RifleShoot(Line::new(x,y,aim,length)));
            },
            GunType::None => (),
        }
    }

    fn gun_update(&self, dt: f64, batch: &Batch, effect_manager: &mut EffectManager) {
        {
            let current_type = self.borrow().gun.gun_type;
            let next_type = self.borrow().gun.next_type;
            if next_type != current_type {
                let loc = self.borrow().location();
                let mut on_armory = false;
                batch.apply_locally(group::ARMORY,&loc, &mut |body: &mut BodyTrait| {
                    on_armory = true;
                });
                if on_armory {
                    self.borrow_mut().gun.gun_type = next_type;
                }
            }
        }

        let mut shoot = false;
        {
            let mut this = self.borrow_mut();
            if this.gun.ammo > 0 {
                if this.gun.reloading > 0. {
                    if this.gun.shooting {
                        this.gun.reloading -= dt;
                    } else {
                        let t = this.gun.reloading - dt;
                        this.gun.reloading = t.max(0.);
                    }
                } else if this.gun.shooting {
                    shoot = true;
                    this.gun.ammo -= 1;
                    this.gun.reloading += this.gun.time_to_reload();
                }
            }
        }
        if shoot {
            self.gun_shoot(batch,effect_manager);
        }
    }
}

const GRENADE_DISTANCE: f64 = 5.;

pub struct Character {
    body: Body,
    aim: f64,
    gun: Gun,
    sword: Sword,
}

pub const WIDTH: f64 = 1.;
pub const VELOCITY: f64 = 65.;
pub const HEIGHT: f64 = 1.;
pub const WEIGHT: f64 = 1.;
pub const MASK: u32 = !0;
pub const GROUP: u32 = super::group::CHARACTER;


impl Character {
    pub fn new(id: usize, x: f64, y: f64, angle: f64) -> Character {
        Character {
            body: Body {
                id: id,
                x: x,
                y: y,
                width: WIDTH,
                height: HEIGHT,
                weight: WEIGHT,
                velocity: 0.,
                angle: angle,
                mask: MASK,
                group: GROUP,
                collision_behavior: CollisionBehavior::Persist,
                body_type: BodyType::Character,
                physic_type: PhysicType::Dynamic,
            },
            aim: angle,
            gun: Gun::new(),
            sword: Sword::new(),
        }
    }

    pub fn render(&mut self, frame_manager: &mut FrameManager) {
        self.body.render(color::RED,frame_manager);
    }
}


pub trait CharacterTrait {
    fn aim(&self) -> f64;
    fn set_aim(&self, a: f64);
    fn set_gun_shoot(&self,bool);
    fn do_sword_attack(&self, batch: &Batch, effect_manager: &mut EffectManager);
    fn set_next_gun_type(&self, next_type: GunType);
    fn launch_grenade(&self,&mut World);
}

impl CharacterTrait for RefCell<Character> {
    fn aim(&self) -> f64 {
        self.borrow().aim
    }

    fn set_aim(&self, a: f64) {
        self.borrow_mut().aim = a;
    }

    fn set_gun_shoot(&self, shoot: bool) {
        self.borrow_mut().gun.shooting = shoot;
    }

    fn set_next_gun_type(&self, next_type: GunType) {
        self.borrow_mut().gun.next_type = next_type;
    }

    fn do_sword_attack(&self, batch: &Batch, effect_manager: &mut EffectManager) {
        self.sword_attack(batch,effect_manager);
    }

    fn launch_grenade(&self,world: &mut World) {
        let aim = self.aim();
        let x = self.borrow().x() + GRENADE_DISTANCE*aim.cos();
        let y = self.borrow().y() + GRENADE_DISTANCE*aim.sin();
        world.insert_grenade(x,y,aim);
    }
}

pub trait CharacterManager {
    fn update(&self, dt: f64, batch: &Batch, effect_manager: &mut EffectManager);
}

impl CharacterManager for RefCell<Character> {
    fn update(&self, dt: f64, batch: &Batch, effect_manager: &mut EffectManager) {
        self.borrow_mut().sword.update(dt);
        self.gun_update(dt,batch,effect_manager);
        let mut this = self.borrow_mut();
        this.body.update(dt);
    }
}

impl BodyTrait for Character {
    delegate!{
        body:
            id() -> usize,
            dead() -> bool,
            body_type() -> BodyType,
            mut damage(d: f64) -> (),
            width() -> f64,
            height() -> f64,
            x() -> f64,
            mut set_x(x: f64) -> (),
            y() -> f64,
            mut set_y(y: f64) -> (),
            weight() -> f64,
            velocity() -> f64,
            mut set_velocity(v: f64) -> (),
            angle() -> f64,
            mut set_angle(a: f64) -> (),
            mask() -> u32,
            group() -> u32,
            collision_behavior() -> CollisionBehavior,
            mut on_collision(other: &mut BodyTrait) -> (),
            physic_type() -> PhysicType,
    }
}
