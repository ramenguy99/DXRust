
use math::{
    vec::{Vec2, Vec3},
    mat::{self, Mat4},
};

use core::f32::consts::PI;

#[derive(Clone, Copy, PartialEq)]
pub enum Direction {
    Right,
    Left,
    Up,
    Down,
    Forward,
    Backward
}

pub struct Camera {
    pub position: Vec3,

    pub right: Vec3,
    pub forward: Vec3,
    pub up: Vec3,

    pub world_up: Vec3,

    pub near: f32,
    pub far: f32,

    pub fov: f32,
    pub aspect_ratio: f32,

    pub move_speed: f32,
    pub rotate_speed: f32,
}

impl Camera {
    pub fn new(position: Vec3, target: Vec3, world_up: Vec3,
            fov: f32, aspect_ratio: f32, near: f32, far: f32,
            move_speed: f32, rotate_speed: f32) -> Camera {
        let mut c = Self {
            position,
            forward: (target - position).normalized(),
            up: Vec3::new(0., 0., 0.),
            right: Vec3::new(0., 0., 0.),
            world_up,
            near,
            far,
            fov,
            aspect_ratio,
            move_speed,
            rotate_speed
        };
        c.update_up_right();
        c
    }

    pub fn view(&self) -> Mat4 {
        mat::lh::look_at(
            self.position,
            self.position + self.forward,
            self.world_up
        )
    }

    pub fn projection(&self) -> Mat4 {
        mat::lh::zo::perspective(
            self.fov, self.near, self.far, self.aspect_ratio,
        )
    }

    pub fn move_in_direction(&mut self, dir: Direction, dt: f32) {
        let delta = dt * self.move_speed;
        match dir {
            Direction::Right    => self.position += self.right     * delta,
            Direction::Left     => self.position -= self.right     * delta,
            Direction::Up       => self.position += self.world_up  * delta,
            Direction::Down     => self.position -= self.world_up  * delta,
            Direction::Forward  => self.position += self.forward   * delta,
            Direction::Backward => self.position -= self.forward   * delta,
        }
    }

    fn update_up_right(&mut self) {
        self.right = self.world_up.cross(self.forward).normalized();
        self.up = self.forward.cross(self.right).normalized();
    }

    pub fn drag(&mut self, offset: Vec2) {
        let (mut theta, mut phi) = Vec3::direction_to_spherical(self.forward);
        theta += f32::clamp(offset.y * self.rotate_speed, -PI * 0.99, PI * 0.99);
        phi += f32::rem_euclid(offset.x * self.rotate_speed, 2. * PI);

        self.forward = Vec3::spherical_to_direction(theta, phi);
        self.update_up_right();
    }
}