use std::fmt::Debug;
use std::ops::Add;
use std::ops::AddAssign;
use std::ops::Div;
use std::ops::DivAssign;
use std::ops::Mul;
use std::ops::MulAssign;
use std::ops::Sub;
use std::ops::SubAssign;

pub trait Scalar:
    Debug
    + Copy
    + Default
    + Add<Output = Self>
    + Mul<Output = Self>
    + Sub<Output = Self>
    + Div<Output = Self>
    + AddAssign
    + MulAssign
    + SubAssign
    + DivAssign
{
}
impl Scalar for f32 {}
impl Scalar for f64 {}
impl Scalar for i32 {}
impl Scalar for i64 {}
impl Scalar for u16 {}
impl Scalar for u32 {}
impl Scalar for u64 {}

#[derive(Clone, Copy, Debug)]
pub struct Vector<T: Scalar, const N: usize>(pub [T; N]);

impl<T: Scalar, const N: usize> Default for Vector<T, N> {
    fn default() -> Self {
        Self([T::default(); N])
    }
}

#[allow(unused, non_camel_case_types)]
pub type vec2 = Vector<f32, 2>;
#[allow(unused, non_camel_case_types)]
pub type vec3 = Vector<f32, 3>;
#[allow(unused, non_camel_case_types)]
pub type vec4 = Vector<f32, 4>;
#[allow(unused, non_camel_case_types)]
pub type ivec2 = Vector<i32, 2>;
#[allow(unused, non_camel_case_types)]
pub type ivec3 = Vector<i32, 3>;
#[allow(unused, non_camel_case_types)]
pub type ivec4 = Vector<i32, 4>;

macro_rules! impl_op_vec {
    ($trait:ident, $op:ident) => {
        impl<T: Scalar, const N: usize> ::std::ops::$trait for Vector<T, N> {
            type Output = Self;
            fn $op(mut self, rhs: Self) -> Self {
                for i in 0..N {
                    self.0[i] = self.0[i].$op(rhs.0[i]);
                }
                self
            }
        }
    };
}

macro_rules! impl_op_assign_vec {
    ($trait:ident, $op:ident) => {
        impl<T: Scalar, const N: usize> ::std::ops::$trait for Vector<T, N> {
            fn $op(&mut self, rhs: Self) {
                for i in 0..N {
                    self.0[i].$op(rhs.0[i]);
                }
            }
        }
    };
}

impl_op_vec!(Add, add);
impl_op_vec!(Sub, sub);
impl_op_vec!(Mul, mul);
impl_op_vec!(Div, div);

impl_op_assign_vec!(AddAssign, add_assign);
impl_op_assign_vec!(SubAssign, sub_assign);
impl_op_assign_vec!(MulAssign, mul_assign);
impl_op_assign_vec!(DivAssign, div_assign);

impl<T: Scalar, const N: usize> Vector<T, N> {
    #[allow(unused)]
    pub fn x(&self) -> T {
        if N < 1 {
            panic!();
        }
        self.0[0]
    }

    #[allow(unused)]
    pub fn y(&self) -> T {
        if N < 2 {
            panic!();
        }
        self.0[1]
    }

    #[allow(unused)]
    pub fn z(&self) -> T {
        if N < 3 {
            panic!();
        }
        self.0[2]
    }

    #[allow(unused)]
    pub fn w(&self) -> T {
        if N < 4 {
            panic!();
        }
        self.0[3]
    }

    #[allow(unused)]
    pub fn x_mut(&mut self) -> &mut T {
        if N < 1 {
            panic!();
        }
        &mut self.0[0]
    }

    #[allow(unused)]
    pub fn y_mut(&mut self) -> &mut T {
        if N < 2 {
            panic!();
        }
        &mut self.0[1]
    }

    #[allow(unused)]
    pub fn z_mut(&mut self) -> &mut T {
        if N < 3 {
            panic!();
        }
        &mut self.0[2]
    }

    #[allow(unused)]
    pub fn w_mut(&mut self) -> &mut T {
        if N < 4 {
            panic!();
        }
        &mut self.0[3]
    }

    #[allow(unused)]
    pub fn dot(self, rhs: Self) -> T {
        let mut sum = T::default();
        for i in 0..N {
            sum += self.0[i] * rhs.0[i];
        }
        sum
    }
}

impl<T: Scalar> Vector<T, 3> {
    #[allow(unused)]
    pub fn cross(self, rhs: Self) -> Self {
        Self([
            self.y() * rhs.z() - self.z() * rhs.y(),
            self.z() * rhs.x() - self.x() * rhs.z(),
            self.x() * rhs.y() - self.y() * rhs.z(),
        ])
    }
}
