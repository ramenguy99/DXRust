use crate::vec::*;

// #[cfg(not(any(feature = "depth_zero_one", feature = "depth_negative_one_one")))]
// compile_error!("no math depth configuration");
//
// #[cfg(all(feature = "depth_zero_one", feature = "depth_negative_one_one"))]
// compile_error!("more than one math depth configuration");


macro_rules! mat_impl {
    ($m: ident, $t: ident, $v: ident, $n: literal) => {

        #[derive(Debug, Default, Copy, Clone)]
        #[repr(C)]
        pub struct $m {
            pub e: [[$t; $n]; $n],
        }

        impl $m {
            #[inline]
            pub fn new() -> $m {
                $m::default()
            }

            #[inline]
            pub fn from_columns(v: &[$v; $n]) -> $m {
                let mut m = $m::new();
                for i in 0..$n {
                    m.e[i] = v[i].to_slice();
                }
                m
            }

            #[inline]
            pub fn identity() -> $m {
                $m::scale_uniform(1.0)
            }

            #[inline]
            pub fn scale_uniform(d: $t) -> $m {
                let mut m = $m::new();
                for i in 0..$n {
                    m.e[i][i] = d;
                }
                m
            }

            #[inline]
            pub fn scale(v: $v) -> $m {
                let vv = v.to_slice();

                let mut m = $m::new();
                for i in 0..$n {
                    m.e[i][i] = vv[i];
                }
                m
            }

            #[inline]
            pub fn transpose(&self) -> $m {
                let mut m = $m::new();

                for j in 0..$n {
                    for i in 0..$n {
                        m.e[j][i] = self.e[i][j];
                    }
                }
                m
            }

            #[inline]
            pub fn to_columns(&self) -> [$v; $n] {
                unsafe {
                    std::mem::transmute_copy::<$m, [$v; $n]>(self)
                }
            }

            #[inline]
            pub fn to_rows(&self) -> [$v; $n] {
                self.transpose().to_columns()
            }

        }

        impl std::ops::Mul<$m> for $m {
            type Output = $m;

            #[inline]
            fn mul(self, rhs: $m) -> $m {
                let mut m = $m::new();

                let a = self.to_rows();
                let b = rhs.to_columns();

                for j in 0..$n {
                    for i in 0..$n {
                        m.e[j][i] = $v::dot(a[i], b[j]);
                    }
                }
                m
            }
        }

        impl std::ops::Mul<$v> for $m {
            type Output = $v;

            #[inline]
            fn mul(self, rhs: $v) -> $v {
                let mut v = [0.0; $n];

                let a = self.to_rows();

                for i in 0..$n {
                    v[i] = a[i].dot(rhs);
                }
                $v::from_slice(&v)
            }
        }


    }
}

mat_impl!(Mat4, f32, Vec4, 4);
mat_impl!(Mat3, f32, Vec3, 3);
mat_impl!(Mat2, f32, Vec2, 2);

mat_impl!(Mat4d, f64, Vec4d, 4);
mat_impl!(Mat3d, f64, Vec3d, 3);
mat_impl!(Mat2d, f64, Vec2d, 2);

impl Mat3 {
    pub fn rotation(axis: Vec3, angle: f32) -> Self {
        let a = axis.x;
        let b = axis.y;
        let c = axis.z;

        let cos_alpha = angle.cos();
        let sin_alpha = angle.sin();

        let k = 1. - cos_alpha;

        let mut m = Mat3::new();
        m.e[0][0] = a * a * k + cos_alpha;
        m.e[1][1] = b * b * k + cos_alpha;
        m.e[2][2] = c * c * k + cos_alpha;

        m.e[0][1] = a * b * k + c * sin_alpha;
        m.e[0][2] = a * c * k - b * sin_alpha;
        m.e[1][2] = b * c * k + a * sin_alpha;

        m.e[1][0] = a * b * k - c * sin_alpha;
        m.e[2][0] = a * c * k + b * sin_alpha;
        m.e[2][1] = b * c * k - a * sin_alpha;

        m
    }
}

impl Mat4 {
    pub fn rotation(axis: Vec3, angle: f32) -> Self {
        let a = axis.x;
        let b = axis.y;
        let c = axis.z;

        let cos_alpha = angle.cos();
        let sin_alpha = angle.sin();

        let k = 1. - cos_alpha;

        let mut m = Mat4::identity();
        m.e[0][0] = a * a * k + cos_alpha;
        m.e[1][1] = b * b * k + cos_alpha;
        m.e[2][2] = c * c * k + cos_alpha;

        m.e[0][1] = a * b * k + c * sin_alpha;
        m.e[0][2] = a * c * k - b * sin_alpha;
        m.e[1][2] = b * c * k + a * sin_alpha;

        m.e[1][0] = a * b * k - c * sin_alpha;
        m.e[2][0] = a * c * k + b * sin_alpha;
        m.e[2][1] = b * c * k - a * sin_alpha;

        m
    }

    pub fn translation(v: Vec3) -> Self {
        let mut m = Mat4::identity();
        m.e[3][0..3].copy_from_slice(&v.to_slice());

        m
    }

    pub fn scale3(v: Vec3) -> Self {
        let vv = v.to_slice();

        let mut m = Mat4::identity();
        for i in 0..3 {
            m.e[i][i] = vv[i];
        }

        m
    }

    // TODO: do this properly after implementing inverse (std::simd?)
    pub fn to_normal_matrix(&self) -> Mat4 {
        let mut m = *self;
        m.e[3][0] = 0.;
        m.e[3][1] = 0.;
        m.e[3][2] = 0.;
        m.e[3][3] = 1.;

        m
    }
}


/// Left-handed matrices
pub mod lh {
    use super::Mat4;
    use super::Vec3;

    pub fn look_at(from: Vec3, to: Vec3, up: Vec3) -> Mat4 {

        let f = (to - from).normalized();
        let r = up.cross(f).normalized();
        let u = f.cross(r);

        let mut m = Mat4::new();
        m.e[0][0] = r.x;
        m.e[1][0] = r.y;
        m.e[2][0] = r.z;

        m.e[0][1] = u.x;
        m.e[1][1] = u.y;
        m.e[2][1] = u.z;

        m.e[0][2] = f.x;
        m.e[1][2] = f.y;
        m.e[2][2] = f.z;

        m.e[0][3] = 0.;
        m.e[1][3] = 0.;
        m.e[2][3] = 0.;

        m.e[3][0] = -Vec3::dot(r, from);
        m.e[3][1] = -Vec3::dot(u, from);
        m.e[3][2] = -Vec3::dot(f, from);
        m.e[3][3] = 1.0;

        m
    }

    // Zero to one z
    pub mod zo {
        use super::super::Mat4;

        pub fn orthographic(left: f32, right: f32, bottom: f32,
                            top: f32, far: f32, near: f32) -> Mat4 {
            let mut m = Mat4::identity();
            m.e[0][0] = 2.0 / (right - left);
            m.e[1][1] = 2.0 / (top - bottom);
            m.e[2][2] = 1.0 / (far - near);
            m.e[3][0] = - (right + left) / (right - left);
            m.e[3][1] = - (top + bottom) / (top - bottom);
            m.e[3][2] = - near / (far - near);

            m
        }


        pub fn perspective(hfov: f32, near: f32, far: f32, aspect_ratio: f32)
            -> Mat4 {

            let mut m = Mat4::new();

            let t = (hfov / 2.).tan();

            m.e[0][0] = 1.0 / t;
            m.e[1][1] = 1.0 / (aspect_ratio * t);
            m.e[2][2] = far / (far - near);
            m.e[2][3] = 1.0;
            m.e[3][2] = -(near * far) / (far - near);

            m
        }

    }

    // Negative one to one z
    pub mod no {
        use super::super::Mat4;

        pub fn orthographic(left: f32, right: f32, bottom: f32,
                            top: f32, far: f32, near: f32) -> Mat4 {
            let mut m = Mat4::identity();
            m.e[0][0] = 2.0 / (right - left);
            m.e[1][1] = 2.0 / (top - bottom);
            m.e[2][2] = 2.0 / (far - near);
            m.e[3][0] = - (right + left) / (right - left);
            m.e[3][1] = - (top + bottom) / (top - bottom);
            m.e[3][2] = - (far + near) / (far - near);

            m
        }
    }
}



