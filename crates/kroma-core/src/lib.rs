//! # Kroma core
//!
//! The headless image-editing engine behind the Kroma editor and CLI. Everything here is
//! pure Rust, multi-threaded with rayon, and independent of any GUI toolkit.

pub mod adjust;
pub mod blend;
pub mod brush;
pub mod buffer;
pub mod color;
pub mod document;
pub mod effects;
pub mod filter;
pub mod gradient;
pub mod history;
pub mod inpaint;
pub mod io;
pub mod layer;
pub mod ops;
pub mod selection;
pub mod shapes;
pub mod text;
pub mod transform;

pub use buffer::{Image, Mask, Rect};
pub use color::Color;
pub use document::Document;
pub use layer::{Layer, LayerId, LayerKind};
