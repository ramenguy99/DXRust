use core::ops;
use core::fmt;
use crate::vec::{Vec3, Vec4, Vec3d, Vec4d};
use crate::mat::{Mat3, Mat3d, Mat4, Mat4d};

use bytemuck::{Pod, Zeroable};

// TODO:
// [ ] from_euler, from_mat3, from_mat4
// [ ] slerp / nlerp
//
// [ ] transform.rs

macro_rules! quat_impl {
    ($name: ident, $t: ident, $v3: ident, $v: ident, $m: ident, $m4: ident) => {

        #[derive(Debug, Default, Copy, Clone, Pod, Zeroable)]
        #[repr(C)]
        pub struct $name {
            pub x: $t,
            pub y: $t,
            pub z: $t,
            pub w: $t,
        }

        impl $name {
            #[inline]
            pub fn new(x: $t, y: $t, z: $t, w: $t) -> Self {
                Self { x, y, z, w }
            }

            #[inline]
            pub fn re(&self) -> $t {
                self.w
            }

            #[inline]
            pub fn im(&self) -> $v3 {
                $v3 { x: self.x, y: self.y, z: self.z }
            }

            #[inline]
            pub fn from_slice(a: &[$t; 4]) -> Self {
                unsafe {
                    std::mem::transmute_copy::<[$t; 4], Self>(a)
                }
            }

            pub fn from_vec4(v: $v) -> Self {
                Self { x: v.x, y: v.y, z: v.z, w: v.w }
            }

            #[inline]
            pub fn to_slice(self) -> [$t; 4] {
                unsafe {
                    std::mem::transmute_copy::<Self, [$t; 4]>(&self)
                }
            }

            #[inline]
            pub fn conj(self) -> Self {
                $name {
                    x: -self.x,
                    y: -self.y,
                    z: -self.z,
                    w: self.w,
                }
            }

            #[inline]
            pub fn norm2(self) -> $t {
                self.x * self.x + self.y * self.y + self.z * self.z * self.w * self.w
            }

            #[inline]
            pub fn norm(self) -> $t {
                self.norm2().sqrt()
            }

            #[inline]
            pub fn normalized(self) -> Self {
                let i = 1.0 / self.norm();
                Self {
                    x: self.x * i,
                    y: self.y * i,
                    z: self.z * i,
                    w: self.w * i,
                }
            }

            #[inline]
            pub fn rotate(axis: $v3, angle: $t) -> Self {
                let s = (angle * 0.5).sin();
                Self {
                    w: (angle * 0.5).cos(),
                    x: axis.x * s,
                    y: axis.y * s,
                    z: axis.z * s,
                }
            }

            #[inline]
            pub fn to_mat3(self) -> $m {
                let x = self.x;
                let y = self.y;
                let z = self.z;
                let w = self.w;

                let xy = x * y;
                let xz = x * z;
                let xw = x * w;
                let yz = y * z;
                let yw = y * w;
                let zw = z * w;
                let x_squared = x * x;
                let y_squared = y * y;
                let z_squared = z * z;

                let mut m = $m::new();
                m.e[0][0] = 1. - 2. * (y_squared + z_squared);
                m.e[1][0] = 2. * (xy - zw);
                m.e[2][0] = 2. * (xz + yw);

                m.e[0][1] = 2. * (xy + zw);
                m.e[1][1] = 1. - 2. * (x_squared + z_squared);
                m.e[2][1] = 2. * (yz - xw);

                m.e[0][2] = 2. * (xz - yw);
                m.e[1][2] = 2. * (yz + xw);
                m.e[2][2] = 1. - 2. * (x_squared + y_squared);

                m
            }

            #[inline]
            pub fn to_mat4(self) -> $m4 {
                let x = self.x;
                let y = self.y;
                let z = self.z;
                let w = self.w;

                let xy = x * y;
                let xz = x * z;
                let xw = x * w;
                let yz = y * z;
                let yw = y * w;
                let zw = z * w;
                let x_squared = x * x;
                let y_squared = y * y;
                let z_squared = z * z;

                let mut m = $m4::identity();
                m.e[0][0] = 1. - 2. * (y_squared + z_squared);
                m.e[1][0] = 2. * (xy - zw);
                m.e[2][0] = 2. * (xz + yw);

                m.e[0][1] = 2. * (xy + zw);
                m.e[1][1] = 1. - 2. * (x_squared + z_squared);
                m.e[2][1] = 2. * (yz - xw);

                m.e[0][2] = 2. * (xz - yw);
                m.e[1][2] = 2. * (yz + xw);
                m.e[2][2] = 1. - 2. * (x_squared + y_squared);

                m
            }


        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "{}(w: {:.prec$}, x: {:.prec$}, y: {:.prec$}, z: {:.prec$})",
                       stringify!($name), self.w, self.x, self.y, self.z,
                       prec = f.precision().unwrap_or(3))
            }
        }

        impl ops::Mul<$name> for $t {
            type Output = $name;

            #[inline]
            fn mul(self, rhs: $name) -> $name {
                $name {
                    x: rhs.x * self,
                    y: rhs.y * self,
                    z: rhs.z * self,
                    w: rhs.w * self }
            }
        }

        impl ops::Mul<$name> for $name {
            type Output = $name;

            #[inline]
            fn mul(self, rhs: $name) -> $name {
                let a = self.im();
                let b = rhs.im();

                let w = self.w * rhs.w - a.dot(b);
                let v = self.w * b + rhs.w * a + a.cross(b);
                $name {
                    x: v.x,
                    y: v.y,
                    z: v.z,
                    w: w
                }
            }
        }

        impl ops::Mul<$v3> for $name {
            type Output = $v3;

            #[inline]
            fn mul(self, rhs: $v3) -> $v3 {
                let s = self.re();
                let v = self.im();

                2. * v.dot(rhs) * v + (s * s - 1.) * rhs - 2. * s * v.cross(rhs)
            }
        }

        impl ops::Mul<$m> for $name {
            type Output = $m;

            #[inline]
            fn mul(self, rhs: $m) -> $m  {
                self.to_mat3() * rhs
            }
        }



    }
}

quat_impl!(Quat, f32, Vec3, Vec4, Mat3, Mat4);
quat_impl!(Quatd, f64, Vec3d, Vec4d, Mat3d, Mat4d);



