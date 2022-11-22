use core::ops;
use core::fmt;

macro_rules! vec_op_impl {
    ($trait: ident, $func: ident, $v: ident, $($e: ident),*) => {
        impl ops::$trait<$v> for $v {
            type Output = $v;

            #[inline]
            fn $func(self, rhs: $v) -> $v {
                $v { $( $e: self.$e.$func(rhs.$e), )* }
            }
        }
    }
}

macro_rules! vec_assign_op_impl {
    ($trait: ident, $func: ident, $v: ident, $($e: ident),*) => {
        impl ops::$trait<$v> for $v {
            #[inline]
            fn $func(&mut self, rhs: $v) {
                $( self.$e.$func(rhs.$e); )*
            }
        }
    }
}

macro_rules! scalar_op_impl {
    ($trait: ident, $func: ident, $v: ident, $t: ident, $($e: ident),*) => {

        impl ops::$trait<$t> for $v {
            type Output = $v;

            #[inline]
            fn $func(self, rhs: $t) -> $v {
                $v { $( $e: self.$e.$func(rhs), )* }
            }
        }

        impl ops::$trait<$v> for $t {
            type Output = $v;

            #[inline]
            fn $func(self, rhs: $v) -> $v {
                $v { $( $e: self.$func(rhs.$e), )* }
            }
        }
    }
}

macro_rules! scalar_assign_op_impl {
    ($trait: ident, $func: ident, $v: ident, $t: ident, $($e: ident),*) => {

        impl ops::$trait<$t> for $v {
            #[inline]
            fn $func(&mut self, rhs: $t) {
                $( self.$e.$func(rhs); )*
            }
        }
    }
}

macro_rules! vec_float_utils_impl {
    ($v: ident, $t: ident, $($e: ident),*) => {
        impl $v {
            #[inline]
            pub fn dot(self, b: $v) -> $t {
                // Adding negative zero (-0.0) is a nop in IEEE 754 floating
                // point, while adding positive zero can change the sign of
                // negative zero, thus llvm only optimizes out (-0.0):
                //
                // (-0.0 + -0.0) = -0.0
                // (-0.0 +  0.0) =  0.0
                // ( 0.0 + -0.0) =  0.0
                // ( 0.0 +  0.0) =  0.0
                $( self.$e * b.$e + )* (-0.0)
            }

            #[inline]
            pub fn length2(self) -> $t {
                $v::dot(self, self)
            }

            #[inline]
            pub fn length(self) -> $t {
                $v::length2(self).sqrt()
            }

            #[inline]
            pub fn norm(self) -> $t {
                $v::length(self)
            }

            #[inline]
            pub fn normalized(self) -> $v {
                self * (1.0 / $v::length(self))
            }

            #[inline]
            pub fn lerp(self, b: $v, t: $t) -> $v {
                $v { $( $e: self.$e * t + b.$e * (1.0 - t), )* }
            }

            #[inline]
            pub fn bounce(self, n: $v) -> $v {
                self - 2.0 * $v::dot(self, n) * n
            }
        }

        impl std::ops::Neg for $v {
            type Output = $v;

            fn neg(self) -> $v {
                $v { $( $e: self.$e.neg(), )* }
            }
        }
    }
}

macro_rules! vec3_utils_impl {
    ($v: ident, $t: ident) => {
        impl $v {
            #[inline]
            pub fn cross(self, b:$v) -> $v {
                $v {
                    x: self.y * b.z - self.z * b.y,
                    y: self.z * b.x - self.x * b.z,
                    z: self.x * b.y - self.y * b.x,
                }
            }
        }
    }
}

macro_rules! vec_impl {
    ($v: ident, $t: ident, $n: expr, $($e: ident),*) => {

        #[derive(Debug, Default, Copy, Clone)]
        #[repr(C)]
        pub struct $v {
            $( pub $e : $t, )*
        }

        impl $v {
            #[inline]
            pub fn new($( $e: $t, )*) -> $v {
                $v { $( $e : $e, )* }
            }

            #[inline]
            pub fn from_scalar(a: $t) -> $v {
                $v { $( $e : a, )* }
            }

            #[inline]
            pub fn from_slice(a: &[$t; $n]) -> $v {
                unsafe {
                    std::mem::transmute_copy::<[$t; $n], $v>(a)
                }
            }

            #[inline]
            pub fn clamp(a: $v, min: $v, max: $v) -> $v {
                $v { $( $e: a.$e.clamp(min.$e, max.$e),)* }
            }

            #[inline]
            pub fn min(a: $v, b: $v) -> $v {
                $v { $( $e: a.$e.min(b.$e),)* }
            }

            #[inline]
            pub fn max(a: $v, b: $v) -> $v {
                $v { $( $e: a.$e.max(b.$e),)* }
            }

            #[inline]
            pub fn to_slice(self) -> [$t; $n] {
                unsafe {
                    std::mem::transmute_copy::<$v, [$t; $n]>(&self)
                }
            }

        }

        impl fmt::Display for $v {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "{}({})", stringify!($v),
                        vec![$(
                           format!("{:.prec$}", self.$e, prec = f.precision().unwrap_or(3)),
                        )*].join(", "))
            }
        }


        vec_op_impl!(Add, add, $v, $($e),*);
        vec_op_impl!(Sub, sub, $v, $($e),*);
        vec_op_impl!(Mul, mul, $v, $($e),*);
        vec_op_impl!(Div, div, $v, $($e),*);

        vec_assign_op_impl!(AddAssign, add_assign, $v, $($e),*);
        vec_assign_op_impl!(SubAssign, sub_assign, $v, $($e),*);
        vec_assign_op_impl!(MulAssign, mul_assign, $v, $($e),*);
        vec_assign_op_impl!(DivAssign, div_assign, $v, $($e),*);

        scalar_op_impl!(Add, add, $v, $t, $($e),*);
        scalar_op_impl!(Sub, sub, $v, $t, $($e),*);
        scalar_op_impl!(Mul, mul, $v, $t, $($e),*);
        scalar_op_impl!(Div, div, $v, $t, $($e),*);

        scalar_assign_op_impl!(AddAssign, add_assign, $v, $t, $($e),*);
        scalar_assign_op_impl!(SubAssign, sub_assign, $v, $t, $($e),*);
        scalar_assign_op_impl!(MulAssign, mul_assign, $v, $t, $($e),*);
        scalar_assign_op_impl!(DivAssign, div_assign, $v, $t, $($e),*);
    }
}


vec_impl!(Vec2i, i32, 2, x, y);
vec_impl!(Vec3i, i32, 3, x, y, z);
vec_impl!(Vec4i, i32, 4, x, y, z, w);

vec_impl!(Vec2u, u32, 2, x, y);
vec_impl!(Vec3u, u32, 3, x, y, z);
vec_impl!(Vec4u, u64, 4, x, y, z, w);

vec_impl!(Vec2, f32, 2, x, y);
vec_impl!(Vec3, f32, 3, x, y, z);
vec_impl!(Vec4, f32, 4, x, y, z, w);

vec_impl!(Vec2d, f64, 2, x, y);
vec_impl!(Vec3d, f64, 3, x, y, z);
vec_impl!(Vec4d, f64, 4, x, y, z, w);

vec_float_utils_impl!(Vec2, f32, x, y);
vec_float_utils_impl!(Vec3, f32, x, y, z);
vec_float_utils_impl!(Vec4, f32, x, y, z, w);
vec_float_utils_impl!(Vec2d, f64, x, y);
vec_float_utils_impl!(Vec3d, f64, x, y, z);
vec_float_utils_impl!(Vec4d, f64, x, y, z, w);

vec3_utils_impl!(Vec3, f32);
vec3_utils_impl!(Vec3d, f64);


