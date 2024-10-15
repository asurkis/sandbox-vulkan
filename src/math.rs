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

#[derive(Clone, Copy, Debug)]
pub struct Vector<T: Scalar, const N: usize>(pub [T; N]);

#[derive(Clone, Copy, Debug)]
pub struct Matrix<T: Scalar, const N: usize, const M: usize>(
    // Column-first to coerce with vector
    pub [[T; N]; M],
);

impl Scalar for f32 {}
impl Scalar for f64 {}
impl Scalar for i16 {}
impl Scalar for i32 {}
impl Scalar for i64 {}
impl Scalar for u16 {}
impl Scalar for u32 {}
impl Scalar for u64 {}

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
#[allow(unused, non_camel_case_types)]
pub type mat4x4 = Matrix<f32, 4, 4>;
#[allow(unused, non_camel_case_types)]
pub type mat4 = mat4x4;

macro_rules! impl_scalar_op {
    ($trait:ident, $op:ident) => {
        impl<T: Scalar, const N: usize> ::std::ops::$trait<T> for Vector<T, N> {
            type Output = Self;
            fn $op(mut self, rhs: T) -> Self {
                for i in 0..N {
                    self.0[i] = self.0[i].$op(rhs);
                }
                self
            }
        }

        impl<T: Scalar, const N: usize, const M: usize> ::std::ops::$trait<T> for Matrix<T, N, M> {
            type Output = Self;
            fn $op(mut self, rhs: T) -> Self {
                for col in 0..M {
                    for row in 0..N {
                        self.0[col][row] = self.0[col][row].$op(rhs);
                    }
                }
                self
            }
        }
    };
}

macro_rules! impl_op {
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

        impl<T: Scalar, const N: usize, const M: usize> ::std::ops::$trait for Matrix<T, N, M> {
            type Output = Self;
            fn $op(mut self, rhs: Self) -> Self {
                for col in 0..M {
                    for row in 0..N {
                        self.0[col][row] = self.0[col][row].$op(rhs.0[col][row]);
                    }
                }
                self
            }
        }
    };
}

macro_rules! impl_op_assign {
    ($trait:ident, $op:ident) => {
        impl<T: Scalar, const N: usize> ::std::ops::$trait for Vector<T, N> {
            fn $op(&mut self, rhs: Self) {
                for i in 0..N {
                    self.0[i].$op(rhs.0[i]);
                }
            }
        }

        impl<T: Scalar, const N: usize, const M: usize> ::std::ops::$trait for Matrix<T, N, M> {
            fn $op(&mut self, rhs: Self) {
                for col in 0..M {
                    for row in 0..N {
                        self.0[col][row].$op(rhs.0[col][row]);
                    }
                }
            }
        }
    };
}

macro_rules! impl_fty {
    ($fty:ty) => {
        impl<const N: usize> Vector<$fty, N> {
            #[allow(unused)]
            pub fn length(self) -> $fty {
                self.dot(self).sqrt()
            }

            #[allow(unused)]
            pub fn normalize(self) -> Self {
                self / self.length()
            }
        }

        impl<const N: usize> Matrix<$fty, N, N> {
            #[allow(unused)]
            pub fn identity() -> Self {
                let mut out = Self::default();
                for i in 0..N {
                    out.0[i][i] = 1.0;
                }
                out
            }
        }
    };
}

impl_scalar_op!(Add, add);
impl_scalar_op!(Mul, mul);
impl_scalar_op!(Sub, sub);
impl_scalar_op!(Div, div);

impl_op!(Add, add);
impl_op!(Sub, sub);
impl_op!(Mul, mul);
impl_op!(Div, div);

impl_op_assign!(AddAssign, add_assign);
impl_op_assign!(SubAssign, sub_assign);
impl_op_assign!(MulAssign, mul_assign);
impl_op_assign!(DivAssign, div_assign);

impl_fty!(f32);
impl_fty!(f64);

impl<T: Scalar, const N: usize> Default for Vector<T, N> {
    fn default() -> Self {
        Self([T::default(); N])
    }
}

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

impl<T: Scalar, const N: usize, const M: usize> Default for Matrix<T, N, M> {
    fn default() -> Self {
        Self([[T::default(); N]; M])
    }
}

impl<T: Scalar, const N: usize, const M: usize> Matrix<T, N, M> {
    #[allow(unused)]
    pub fn transpose(&self) -> Matrix<T, M, N> {
        let mut result = Matrix::default();
        for col in 0..M {
            for row in 0..N {
                result.0[row][col] = self.0[col][row];
            }
        }
        result
    }

    #[allow(unused)]
    pub fn dot<const K: usize>(&self, rhs: &Matrix<T, M, K>) -> Matrix<T, N, K> {
        let mut result = Matrix::default();
        for col in 0..K {
            for row in 0..N {
                for i in 0..M {
                    result.0[col][row] += self.0[i][row] * rhs.0[col][i];
                }
            }
        }
        result
    }

    #[allow(unused)]
    pub fn dotv(&self, rhs: Vector<T, M>) -> Vector<T, N> {
        self.dot(&rhs.into()).into()
    }

    #[allow(unused)]
    pub fn dot_assign(&mut self, rhs: &Matrix<T, M, M>) {
        *self = self.dot(rhs);
    }
}

impl<T: Scalar, const N: usize> Matrix<T, N, N> {
    #[allow(unused)]
    pub fn transpose_inplace(&mut self) {
        for col in 0..N {
            for row in 0..col {
                (self.0[row][col], self.0[col][row]) = (self.0[col][row], self.0[row][col]);
            }
        }
    }
}

impl<T: Scalar, const N: usize> From<T> for Vector<T, N> {
    fn from(value: T) -> Self {
        Self([value; N])
    }
}

impl<T: Scalar, const N: usize, const M: usize> From<T> for Matrix<T, N, M> {
    fn from(value: T) -> Self {
        Self([[value; N]; M])
    }
}

impl<T: Scalar, const N: usize> From<Matrix<T, N, 1>> for Vector<T, N> {
    fn from(value: Matrix<T, N, 1>) -> Self {
        Self(value.0[0])
    }
}

impl<T: Scalar, const N: usize> From<Vector<T, N>> for Matrix<T, N, 1> {
    fn from(value: Vector<T, N>) -> Self {
        Matrix([value.0])
    }
}
