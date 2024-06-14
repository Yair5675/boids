use ggez::graphics::Color;

pub const PROGRAM_NAME: &str = "Boids Sim";
pub const AUTHOR: &str = "Yair Ziv";

pub const SCREEN_WIDTH: f32 = 1400f32;
pub const SCREEN_HEIGHT: f32 = 1000f32;

pub const FPS: u32 = 60;

// Boids parameters:
pub const BOIDS_NUM: usize = 800;
pub const MAX_BOID_VELOCITY: f32 = 6.;
pub const MIN_BOID_VELOCITY: f32 = 5.;
pub const BOID_COLORS: [Color; 7] = [
    Color::BLACK, Color::YELLOW, Color::BLUE, Color::MAGENTA, Color::GREEN, Color::RED, Color::CYAN
];

// Parameters for boid rules:
pub const SEPARATION_FACTOR: f32 = 0.1;
pub const ALIGNMENT_FACTOR: f32 = 0.05;
pub const COHESION_FACTOR: f32 = 0.005;
pub const EVASION_FACTOR: f32 = 1.3;
pub const TARGET_FACTOR: f32 = 0.0005;
pub const LEADER_FACTOR: f32 = 0.0005;

// Margin from window walls until evasion comes into play:
pub const MARGIN: f32 = SCREEN_WIDTH / 10.;

// Boids close to others will influence their direction. This is the maximum influence distance:
pub const STEERING_DISTANCE: f32 = 25.;
pub const STEERING_DISTANCE_SQUARED: f32 = STEERING_DISTANCE * STEERING_DISTANCE;
pub const INFLUENCE_DISTANCE: f32 = 75.;
pub const INFLUENCE_DISTANCE_SQUARED: f32 = INFLUENCE_DISTANCE * INFLUENCE_DISTANCE;
pub const LOCATION_GRID_HEIGHT: usize = (SCREEN_HEIGHT / INFLUENCE_DISTANCE) as usize + 1;
pub const LOCATION_GRID_WIDTH: usize = (SCREEN_WIDTH / INFLUENCE_DISTANCE) as usize + 1;