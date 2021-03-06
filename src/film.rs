use bmp;
use cast::{usize, u32, u8};
use itertools::{Itertools, MinMaxResult};
use ordered_float::NotNaN;
use rayon::prelude::*;
use std::{f32, iter, slice};

pub struct Frame<T> {
    width: u32,
    height: u32,
    buffer: Vec<T>,
}

impl<T: Sync + Send + Copy> Frame<T> {
    pub fn new(width: u32, height: u32, value: T) -> Self {
        Frame {
            width,
            height,
            buffer: vec![value; usize(width) * usize(height)],
        }
    }

    pub fn for_each_pixel<F>(&self, mut f: F)
        where F: FnMut(u32, u32, T)
    {
        for (i, px) in self.pixel_values().enumerate() {
            // TODO why height and not width?
            // TODO iterate differently to avoid the divmod
            let x = u32(i).unwrap() / self.height;
            let y = u32(i).unwrap() % self.height;
            f(x, y, px)
        }
    }

    pub fn set_pixels<F>(&mut self, f: F)
        where F: Send + Sync + Fn(u32, u32) -> T
    {
        // TODO why height and not width?
        let height = self.height;
        self.buffer[..]
            .par_iter_mut()
            .enumerate()
            // TODO iterate differently to avoid the divmod
            .for_each(move |(i, px)| {
                let x = u32(i).unwrap() / height;
                let y = u32(i).unwrap() % height;
                *px = f(x, y);
            });
    }

    fn pixel_values(&self) -> iter::Cloned<slice::Iter<T>>
        where T: Copy
    {
        self.buffer.iter().cloned()
    }

    fn to_bmp<F>(&self, f: F) -> bmp::Image
        where F: Fn(T) -> bmp::Pixel
    {
        let mut img = bmp::Image::new(self.width, self.height);
        self.for_each_pixel(|x, y, px| { img.set_pixel(x, y, f(px)); });
        img
    }
}

/// Compute the linear interpolation coefficient for producing x from x0 and x1, i.e.,
/// the scalar t \in [0, 1] such that x = (1 - t) * x0 + t * x1
/// Panics if this is not possible, i.e., x is not between x0 and x1.
fn inv_lerp<T: Copy + Into<f64> + PartialOrd>(x: T, x0: T, x1: T) -> f64 {
    assert!(x0 <= x && x <= x1);
    let t = (x.into() - x0.into()) / (x1.into() - x0.into());
    debug_assert!(0.0 <= t && t <= 1.0);
    t
}

pub trait ToBmp {
    fn to_bmp(&self) -> bmp::Image;
}

pub struct Depthmap(pub Frame<f32>);
pub struct Heatmap(pub Frame<u32>);

impl ToBmp for Depthmap {
    fn to_bmp(&self) -> bmp::Image {
        let frame = &self.0;
        let (min_depth, max_depth) = match frame.pixel_values()
                  .filter(|&x| x != f32::INFINITY)
                  .minmax_by_key(|&x| NotNaN::new(x).unwrap()) {
            MinMaxResult::MinMax(min, max) => (min, max),
            _ => panic!("frame empty or not a single pixel"),
        };
        frame.to_bmp(|depth| if depth == f32::INFINITY {
                         bmp::consts::BLUE
                     } else {
                         let intensity = inv_lerp(depth, min_depth, max_depth);
                         let s = u8(((1.0 - intensity) * 255.0).round()).unwrap();
                         bmp::Pixel { r: s, g: s, b: s }
                     })
    }
}

impl ToBmp for Heatmap {
    fn to_bmp(&self) -> bmp::Image {
        let frame = &self.0;
        let (min_heat, max_heat) = match frame.pixel_values().minmax() {
            MinMaxResult::MinMax(min, max) => (min, max),
            _ => panic!("frame empty or a single pixel"),
        };
        frame.to_bmp(|heat| {
                         let intensity = inv_lerp(heat, min_heat, max_heat);
                         let s = u8((intensity * 255.0).round()).unwrap();
                         bmp::Pixel { r: s, g: 0, b: 0 }
                     })
    }
}
