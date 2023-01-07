use crate::{
    board::{RustrisBoard, SlotState, TranslationDirection},
    controls::{ControlStates, Controls, InputState},
    rustomino::*,
    view::{self, ViewSettings},
    VIEW_DIMENSIONS,
};
use std::f64::consts::E;
use strum::IntoEnumIterator;

use macroquad::{prelude::*, rand::ChooseRandom};

const GRAVITY_NUMERATOR: f64 = 1.0; // how
const GRAVITY_FACTOR: f64 = 2.0; // slow or increase gravity factor
const LINES_PER_LEVEL: usize = 10; // how many blocks between levels (should this be score based?)

// const DEBUG_RNG_SEED: u64 = 123456789; // for debugging RNG
// const DELAY_TO_LOCK: f64 = 0.5; // how long to wait before locking a block which cannot move down
// const MAX_DELAY_RESETS: i32 = 10; // how many times to reset the delay

const SINGLE_LINE_SCORE: usize = 100;
const DOUBLE_LINE_SCORE: usize = 300;
const TRIPLE_LINE_SCORE: usize = 500;
const RUSTRIS_SCORE: usize = 800;

pub enum GameState {
    Menu,
    Playing,
    Paused,
    GameOver,
}

/// returns the delay for the level in fractional seconds
fn gravity_delay(level: usize) -> f64 {
    let gravity_delay =
        (GRAVITY_NUMERATOR / (((level + 1) as f64).log(E) * GRAVITY_FACTOR)).max(0.001);
    log::info!("new gravity_delay {}", gravity_delay);
    gravity_delay
}

pub struct RustrisGame {
    pub board: RustrisBoard,
    pub next_rustomino: Option<Rustomino>,
    pub held_rustomino: Option<Rustomino>,
    pub game_state: GameState,
    pub score: usize,
    pub game_level: usize,
    rustomino_bag: Vec<RustominoType>,
    gravity_time_accum: f64,
    gravity_delay: f64,
    completed_lines: usize,
    last_update: f64,
    view_settings: ViewSettings,
    hold_used: bool,
}

impl RustrisGame {
    pub fn new(board: RustrisBoard, view_settings: ViewSettings) -> Self {
        RustrisGame {
            board,
            next_rustomino: None,
            held_rustomino: None,
            game_state: GameState::Menu, // GameState::Menu,
            score: 0,
            game_level: 1,
            hold_used: false,
            rustomino_bag: Vec::new(),
            gravity_time_accum: 0.0,
            gravity_delay: gravity_delay(1),
            completed_lines: 0,
            last_update: get_time(),
            view_settings,
        }
        .init()
    }

    fn init(mut self) -> Self {
        log::info!("Initializing RustrisGame");
        self.get_next_rustomino();
        self
    }

    fn increase_game_level(&mut self) {
        self.game_level += 1;
        log::info!("increasing game level to {}", self.game_level);
        self.gravity_delay = gravity_delay(self.game_level);
    }

    fn get_next_rustomino(&mut self) {
        // this can be called even if next_rustomino is some
        // in this case do nothing
        if self.next_rustomino.is_some() {
            return;
        }

        // if we've used all of the rustomino's fill the bag
        self.fill_rustomino_bag();

        if let Some(next_type) = self.rustomino_bag.pop() {
            log::debug!("next rustomino: {:?}", next_type);
            self.next_rustomino = Some(Rustomino::new(next_type));
        }
    }

    // add one of each rustomino type to bag
    // then shuffle the bag
    fn fill_rustomino_bag(&mut self) {
        if !self.rustomino_bag.is_empty() {
            log::debug!("rustomino bag: {:?}", self.rustomino_bag);
            return;
        }
        self.rustomino_bag
            .append(&mut RustominoType::iter().collect());
        self.rustomino_bag.shuffle();
        log::debug!("filled rustomino bag: {:?}", self.rustomino_bag);
    }

    fn gravity_tick(&mut self) {
        // check to see if the board's current rustomino can fall
        let movable = self.board.can_fall();

        log::debug!("board:\n{}", self.board);
        log::debug!("gravity tick, rustomino movable: {movable}");

        if movable {
            self.board.apply_gravity();
        } else {
            self.lock("gravity tick");
        }
    }

    fn lock(&mut self, reason: &str) {
        if let Some(rustomino) = &self.board.current_rustomino {
            log::info!(
                "locking rustomnio for {reason}; type: {:?} blocks: {:?}",
                rustomino.rustomino_type,
                rustomino.board_slots()
            );
        }
        self.hold_used = false;
        self.board.lock_rustomino();

        self.handle_completed_lines();
    }

    fn translate(&mut self, direction: TranslationDirection) {
        self.board.translate_rustomino(direction);
    }

    fn rotate(&mut self, direction: RotationDirection) {
        self.board.rotate_rustomino(direction);
    }

    fn soft_drop(&mut self) {
        if !self.board.translate_rustomino(TranslationDirection::Down) {
            self.lock("soft drop");
        }
        self.gravity_time_accum = 0.0;
    }

    fn hard_drop(&mut self) {
        self.board.hard_drop();
        self.lock("hard drop");
        self.gravity_time_accum = 0.0;
    }

    // Hold action. Hold a rustomino for later use.
    // If a rustomino has not yet been held, the current rustomino is held,
    // and the next rustomino is added to the board
    // If a rustomino is already held, this rustomino is added to the board,
    // and the current rustomino is held
    // The player can't use the hold action again until the current rustomino is locked
    fn hold(&mut self) {
        // check to see if the player has used the hold action
        // and they haven't yet locked the rustomino they took
        if self.hold_used {
            return;
        }
        // check to see if there is a held rustomino
        let rustomino = if self.held_rustomino.is_some() {
            // take the held_rustomino
            self.held_rustomino.take().unwrap()
        } else {
            // if not we take the next rustomino
            self.next_rustomino.take().unwrap()
        };

        // if we used next_rustomino we need to replace it
        self.get_next_rustomino();

        // take current_rustomino and make it the hold_rustomino
        self.held_rustomino = Some(self.board.current_rustomino.take().unwrap().reset());
        self.board.set_current_rustomino(rustomino);

        // prevent the player from taking the hold action again
        // until the next rustomino is locked
        self.hold_used = true;
    }

    fn game_over(&mut self) {
        log::info!("Game Over! Score: {}", self.score);
        self.game_state = GameState::GameOver;
    }

    fn handle_completed_lines(&mut self) {
        let completed_lines = self.board.clear_completed_lines();
        if completed_lines.is_empty() {
            return;
        }
        self.completed_lines += completed_lines.len();
        self.score_completed_lines(completed_lines);
    }

    fn score_completed_lines(&mut self, completed_lines: Vec<usize>) {
        // Single line 100xlevel
        // Double line 300xlevel
        // Triple line 500xlevel
        // Rustris (4 lines) 800xlevel
        let score = match completed_lines.len() {
            1 => {
                log::info!("scored! single line");
                SINGLE_LINE_SCORE
            }
            2 => {
                log::info!("scored! double line");
                DOUBLE_LINE_SCORE
            }
            3 => {
                log::info!("scored! triple line");
                TRIPLE_LINE_SCORE
            }
            4 => {
                log::info!("scored! rustris");
                RUSTRIS_SCORE
            }
            _ => {
                panic!("shouldn't be able to score more than 4 l ines")
            }
        };
        let score = score * self.game_level;
        self.score += score;
        log::info!(
            "scored! game_level: {} score: {} total score: {}",
            self.game_level,
            score,
            self.score
        )
    }

    pub fn draw(&self, text_params: &TextParams) {
        match self.game_state {
            GameState::Menu => {
                self.draw_playing_backgound();
                self.draw_menu(text_params);
            }
            GameState::Playing => {
                self.draw_playing_backgound();
                self.draw_playing();
                self.draw_playing_ui(text_params)
            }
            GameState::Paused => {
                self.draw_playing_backgound();
                self.draw_playing();
                self.draw_playing_ui(text_params);
                self.draw_paused(text_params)
            }
            GameState::GameOver => {
                self.draw_playing_backgound();
                self.draw_playing();
                self.draw_playing_ui(text_params);
                self.draw_gameover(text_params)
            }
        }
    }

    fn draw_playing_backgound(&self) {
        draw_rectangle(
            self.view_settings.staging_rect.x,
            self.view_settings.staging_rect.y,
            self.view_settings.staging_rect.w,
            self.view_settings.staging_rect.h,
            view::STAGING_BACKGROUND_COLOR,
        );

        draw_rectangle(
            self.view_settings.board_rect.x,
            self.view_settings.board_rect.y,
            self.view_settings.board_rect.w,
            self.view_settings.board_rect.h,
            view::BOARD_BACKGROUND_COLOR,
        );

        draw_rectangle(
            self.view_settings.preview_rect.x,
            self.view_settings.preview_rect.y,
            self.view_settings.preview_rect.w,
            self.view_settings.preview_rect.h,
            view::PREVIEW_BACKGROUND_COLOR,
        );

        draw_rectangle(
            self.view_settings.hold_rect.x,
            self.view_settings.hold_rect.y,
            self.view_settings.hold_rect.w,
            self.view_settings.hold_rect.h,
            view::HOLD_BACKGROUND_COLOR,
        );
    }

    fn draw_playing(&self) {
        for (y, slots_x) in self.board.slots.iter().enumerate() {
            for (x, slot) in slots_x.iter().enumerate() {
                match slot {
                    SlotState::Locked(rtype) => {
                        // draw the block
                        let rect = board_block_rect([x as i32, y as i32], &self.view_settings);
                        draw_rectangle(rect.x, rect.y, rect.w, rect.h, rtype.color());
                    }
                    _ => {}
                }
            }
        }

        if let Some(next) = &self.next_rustomino {
            for slot in next.blocks {
                // display the preview
                // draw the block
                let rect = next_block_rect([slot[0], slot[1]], &self.view_settings);
                draw_rectangle(rect.x, rect.y, rect.w, rect.h, next.rustomino_type.color());
            }
        }

        if let Some(held) = &self.held_rustomino {
            for slot in held.blocks {
                // display the preview
                // draw the block
                let rect = hold_block_rect([slot[0], slot[1]], &self.view_settings);
                draw_rectangle(rect.x, rect.y, rect.w, rect.h, held.rustomino_type.color());
            }
        }

        if let Some(rustomino) = &self.board.current_rustomino {
            for slot in rustomino.board_slots() {
                // display the preview
                // draw the block
                let rect = board_block_rect([slot[0], slot[1]], &self.view_settings);
                draw_rectangle(
                    rect.x,
                    rect.y,
                    rect.w,
                    rect.h,
                    rustomino.rustomino_type.color(),
                );
            }
        }

        if let Some(ghost) = &self.board.ghost_rustomino {
            for block in ghost.board_slots() {
                // draw the block
                let rect = board_block_rect([block[0], block[1]], &self.view_settings);
                draw_rectangle_lines(rect.x, rect.y, rect.w, rect.h, 4., view::GHOST_COLOR);
            }
        }
    }

    fn draw_playing_ui(&self, text_params: &TextParams) {
        draw_text_ex(
            "Rustris",
            self.view_settings.title_label_pos.x as f32,
            self.view_settings.title_label_pos.y as f32,
            *text_params,
        );

        draw_text_ex(
            "Level:",
            self.view_settings.level_label_pos.x as f32,
            self.view_settings.level_label_pos.y as f32,
            *text_params,
        );

        draw_text_ex(
            &self.game_level.to_string(),
            self.view_settings.level_pos.x as f32,
            self.view_settings.level_pos.y as f32,
            *text_params,
        );

        draw_text_ex(
            "Score:",
            self.view_settings.score_label_pos.x as f32,
            self.view_settings.score_label_pos.y as f32,
            *text_params,
        );

        draw_text_ex(
            &self.score.to_string(),
            self.view_settings.score_pos.x as f32,
            self.view_settings.score_pos.y as f32,
            *text_params,
        );
    }

    fn draw_paused(&self, text_params: &TextParams) {
        draw_rectangle(
            0.,
            0.,
            VIEW_DIMENSIONS[0] as f32,
            VIEW_DIMENSIONS[1] as f32,
            view::PAUSED_OVERLAY_COLOR,
        );
        draw_text_ex(
            "Paused",
            (VIEW_DIMENSIONS[0] / 2 - 55) as f32,
            (VIEW_DIMENSIONS[1] / 2) as f32,
            *text_params,
        );
    }

    fn draw_menu(&self, text_params: &TextParams) {
        draw_rectangle(
            0.,
            0.,
            VIEW_DIMENSIONS[0] as f32,
            VIEW_DIMENSIONS[1] as f32,
            view::PAUSED_OVERLAY_COLOR,
        );
        draw_text_ex(
            "Welcome to Rustris!",
            (VIEW_DIMENSIONS[0] / 2 - 168) as f32,
            (VIEW_DIMENSIONS[1] / 2) as f32,
            *text_params,
        );
        draw_text_ex(
            "Press Enter To Start",
            (VIEW_DIMENSIONS[0] / 2 - 185) as f32,
            (VIEW_DIMENSIONS[1] / 2 + 50) as f32,
            *text_params,
        );
    }

    fn draw_gameover(&self, text_params: &TextParams) {
        draw_rectangle(
            0.,
            0.,
            VIEW_DIMENSIONS[0] as f32,
            VIEW_DIMENSIONS[1] as f32,
            view::PAUSED_OVERLAY_COLOR,
        );
        draw_text_ex(
            "Game Over!",
            (VIEW_DIMENSIONS[0] / 2 - 100) as f32,
            (VIEW_DIMENSIONS[1] / 2) as f32,
            *text_params,
        );
        draw_text_ex(
            "Press Enter To Play Again",
            (VIEW_DIMENSIONS[0] / 2 - 200) as f32,
            (VIEW_DIMENSIONS[1] / 2 + 50) as f32,
            *text_params,
        );
    }

    pub fn update(&mut self, controls: &mut ControlStates) {
        let now = get_time();
        let delta_time = now - self.last_update;

        match self.game_state {
            GameState::Menu => {
                if is_key_pressed(KeyCode::Enter) {
                    self.resume();
                }
            }
            GameState::Playing => {
                // check board ready for the next rustomino
                if self.board.ready_for_next() {
                    // TODO: move this whole block to a fn
                    // take the next rustomino
                    // unwrap should be safe here
                    let current_rustomino = self.next_rustomino.take().unwrap();
                    // we used next_rustomino so we need to replace it
                    self.get_next_rustomino();
                    // add the next rustomino to the board
                    // game over if it can't be placed without a collision
                    if !self.board.set_current_rustomino(current_rustomino) {
                        self.game_over();
                    }
                }

                if is_key_pressed(KeyCode::Escape) {
                    controls.clear_inputs();
                    self.pause();
                }
                self.handle_inputs(controls);
                self.handle_held_inputs(controls, delta_time);
                // Apply "gravity" to move the current rustomino down the board
                // or if it can't move lock it
                self.gravity_time_accum += delta_time;
                if self.gravity_time_accum >= self.gravity_delay {
                    self.gravity_time_accum = 0.0;
                    self.gravity_tick();
                }

                // increase the game level every LINES_PER_LEVEL
                if self.completed_lines > self.game_level * LINES_PER_LEVEL {
                    self.increase_game_level();
                }
            }
            GameState::Paused => {
                if is_key_pressed(KeyCode::Escape) {
                    self.resume();
                }
            }
            GameState::GameOver => {
                if is_key_pressed(KeyCode::Enter) {
                    self.play_again();
                }
            }
        }
        self.last_update = now;
    }

    fn pause(&mut self) {
        self.game_state = GameState::Paused;
    }

    fn resume(&mut self) {
        self.game_state = GameState::Playing;
    }

    fn play_again(&mut self) {
        self.game_state = GameState::Playing;
        self.board = RustrisBoard::new();
        self.next_rustomino = None;
        self.held_rustomino = None;
        self.game_state = GameState::Playing;
        self.score = 0;
        self.game_level = 1;
        self.hold_used = false;
        self.rustomino_bag = Vec::new();
        self.gravity_time_accum = 0.0;
        self.gravity_delay = gravity_delay(1);
        self.completed_lines = 0;
        self.last_update = get_time();
        self.get_next_rustomino();
    }

    fn handle_held_inputs(&mut self, controls: &mut ControlStates, delta_time: f64) {
        // check each input
        for input in Controls::iter() {
            controls
                .input_states
                .entry(input.clone())
                .and_modify(|e| match e {
                    InputState::Down(down_time) => {
                        if let Some(action_delay) = input.action_delay_for_input() {
                            *down_time += delta_time;
                            if *down_time >= action_delay {
                                *e = InputState::Held(0.0);
                            }
                        }
                    }
                    InputState::Held(held_time) => {
                        *held_time += delta_time;
                    }
                    _ => (),
                });
            if let Some(state) = controls.input_states.get_mut(&input) {
                if let InputState::Held(held_time) = state {
                    if let Some(action_repeat_delay) = input.action_repeat_delay_for_input() {
                        if *held_time >= action_repeat_delay {
                            *state = InputState::Held(0.0);
                            match input {
                                Controls::Left => self.translate(TranslationDirection::Left),
                                Controls::Right => self.translate(TranslationDirection::Right),
                                Controls::RotateCW => self.rotate(RotationDirection::Cw),
                                Controls::RotateCCW => self.rotate(RotationDirection::Ccw),
                                Controls::SoftDrop => self.soft_drop(),
                                _ => (),
                            }
                        }
                    }
                }
            }
        }
    }

    fn handle_inputs(&mut self, inputs: &mut ControlStates) {
        for (input, keys) in &inputs.input_map.clone() {
            for key in keys.iter().flatten() {
                if is_key_pressed(*key) {
                    inputs
                        .input_states
                        .entry(input.clone())
                        .and_modify(|e| *e = InputState::Down(0.0));
                    match input {
                        Controls::Left => self.translate(TranslationDirection::Left),
                        Controls::Right => self.translate(TranslationDirection::Right),
                        Controls::RotateCW => self.rotate(RotationDirection::Cw),
                        Controls::RotateCCW => self.rotate(RotationDirection::Ccw),
                        Controls::SoftDrop => self.soft_drop(),
                        Controls::HardDrop => self.hard_drop(),
                        Controls::Hold => self.hold(),
                    }
                } else if is_key_released(*key) {
                    inputs
                        .input_states
                        .entry(input.clone())
                        .and_modify(|e| *e = InputState::Up);
                }
            }
        }
    }
}

fn next_block_rect(block: [i32; 2], settings: &ViewSettings) -> Rect {
    // block[x,y] absolute units
    let x = settings.preview_rect.x
        + (block[0] as f32 * (view::BLOCK_SIZE + view::BLOCK_PADDING) as f32)
        + 1.0;
    // get bottom left of board_rect
    let y = settings.preview_rect.y + settings.preview_rect.h
        - (block[1] as f32 * (view::BLOCK_SIZE + view::BLOCK_PADDING) as f32);

    Rect::new(x, y, view::BLOCK_SIZE as f32, view::BLOCK_SIZE as f32)
}

fn hold_block_rect(block: [i32; 2], settings: &ViewSettings) -> Rect {
    // block[x,y] absolute units
    let x = settings.hold_rect.x
        + (block[0] as f32 * (view::BLOCK_SIZE + view::BLOCK_PADDING) as f32)
        + 1.0;
    // get bottom left of board_rect
    let y = settings.hold_rect.y + settings.hold_rect.h
        - (block[1] as f32 * (view::BLOCK_SIZE + view::BLOCK_PADDING) as f32);

    Rect::new(x, y, view::BLOCK_SIZE as f32, view::BLOCK_SIZE as f32)
}

fn board_block_rect(block: [i32; 2], settings: &ViewSettings) -> Rect {
    // block[x,y] absolute units
    let x = settings.staging_rect.x
        + (block[0] as f32 * (view::BLOCK_SIZE + view::BLOCK_PADDING) as f32)
        + 1.0;
    // get bottom left of board_rect
    let y = settings.board_rect.y + settings.board_rect.h
        - ((block[1] + 1) as f32 * (view::BLOCK_SIZE + view::BLOCK_PADDING) as f32)
        - 1.0;

    Rect::new(x, y, view::BLOCK_SIZE as f32, view::BLOCK_SIZE as f32)
}
