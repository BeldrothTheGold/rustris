use crate::{
    rustominos::RotationDirection,
    rustris_board::RustrisBoard,
    rustris_controller::{RustrisController, RustrisOptions},
    rustris_view::RustrisView,
};
use piston_window::{types::Color, *};
use rustominos::{Rustomino, RustominoType};
use strum::IntoEnumIterator;

mod rustominos;
mod rustris_board;
mod rustris_controller;
mod rustris_view;

const BACKGROUND_COLOR: Color = [0.0, 0.0, 0.0, 1.0];
const WINDOW_DIMENSIONS: [u32; 2] = [1024, 768];

fn main() {
    env_logger::init_from_env("RUSTRIS_LOG_LEVEL");
    log::info!("Startup: Initializing Piston Window");
    let mut window: piston_window::PistonWindow =
        piston_window::WindowSettings::new("Rustris", WINDOW_DIMENSIONS)
            .resizable(false)
            .exit_on_esc(true)
            .vsync(true)
            .build()
            .expect("fatal error, could not create window");

    let rustris_board = RustrisBoard::new();
    let mut rustris_controller = RustrisController::new(rustris_board).init();
    let rustris_view = RustrisView::new();

    while let Some(event) = window.next() {
        if let Some(Button::Keyboard(key)) = event.press_args() {
            rustris_controller.key_pressed(key);
        }
        if let Some(Button::Keyboard(key)) = event.release_args() {
            rustris_controller.key_released(key);
        }
        window.draw_2d(&event, |c, g, _| {
            clear(BACKGROUND_COLOR, g);
            rustris_view.draw(&rustris_controller, &c, g)
        });
        event.update(|arg| {
            rustris_controller.update(arg.dt);
        });
    }
}
