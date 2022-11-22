pub mod vec;
pub mod mat;
pub mod quat;

#[cfg(test)]
mod tests {

    #[test]
    fn test() {
        use crate::vec::Vec2;
        use crate::mat::Mat2;

        let x = Vec2::new(10.0, 2.0);
        let y = Vec2::new(3.0, 3.0);

        let a = Mat2::scale_uniform(3.0);
        let b = Mat2::scale_uniform(2.0) * a;

        let mut z = x;
        z += 1.0;

        let _lhn = crate::mat::lh::no::orthographic(1., 1., 1., 1., 1., 1.);

        println!("{:.2} {}", b * z, Vec2::clamp(y, Vec2::new(5.0, 2.0), x));
    }
}
