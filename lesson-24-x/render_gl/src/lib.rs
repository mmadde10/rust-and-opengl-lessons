extern crate gl;
extern crate half;
extern crate vec_2_10_10_10;
extern crate nalgebra as na;
extern crate ncollide3d;
extern crate resources;
extern crate font_kit;
extern crate euclid;
extern crate image;
extern crate lyon_tessellation;
extern crate lyon_path;
extern crate int_hash;
#[macro_use] extern crate slotmap;
#[macro_use] extern crate failure;
#[macro_use] extern crate lesson_24_x_render_gl_derive as render_gl_derive;

mod flatlander;
mod debug_lines;
mod shader;

pub mod buffer;
pub mod data;
pub mod viewport;
pub mod color_buffer;
pub mod text;

pub use self::viewport::Viewport;
pub use self::color_buffer::ColorBuffer;
pub use self::debug_lines::{DebugLines, RayMarkers, AabbMarker, RectMarker};
pub use self::flatlander::{Flatlander, Alphabet, FlatlanderVertex};
pub use self::shader::{Shader, Program, Error};

