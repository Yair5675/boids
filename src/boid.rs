use std::hash::{Hash, Hasher};
use ggez::{Context, GameResult};
use ggez::glam::Vec2;
use ggez::graphics::{Color, DrawMode, DrawParam, Mesh};
use ggez::mint::Point2;
use ordered_float::OrderedFloat;
use crate::constants::{MAX_BOID_VELOCITY, MIN_BOID_VELOCITY, SCREEN_HEIGHT, SCREEN_WIDTH};

pub struct Boid {
    pos: Vec2,
    speed: Vec2,
    color: Color
}

impl Boid {
    pub fn new(initial_x: f32, initial_y: f32, color: Color) -> Self {
        Self {
            pos: Vec2::new(initial_x, initial_y),
            speed: Vec2::ONE * MAX_BOID_VELOCITY / 2.,
            color
        }
    }

    pub fn go_forward(&mut self) {
        self.pos += self.speed;

        // Fix position on screen:
        self.pos.x = self.pos.x.rem_euclid(SCREEN_WIDTH);
        self.pos.y = self.pos.y.rem_euclid(SCREEN_HEIGHT);
    }

    /// All boids are drawn in the same shape (rotated to match their path of course). This method
    /// returns that shape as a Mesh for rendering. The mesh is not located anywhere in particular
    /// and upon rendering it using DrawParams will be necessary to move it to the desired location.
    pub fn get_boid_mesh(context: &Context) -> GameResult<Mesh> {
        // Get the triangles vertexes:
        const SHAPE_STRETCHER: f32 = 5.;
        let mesh_points = vec![
            Point2 { x: -SHAPE_STRETCHER, y: SHAPE_STRETCHER },
            Point2 { x: -SHAPE_STRETCHER, y: -SHAPE_STRETCHER },
            Point2 { x: 1.5 * SHAPE_STRETCHER, y: 0. }
        ];

        // Create the mesh:
        Mesh::new_polygon(
            context,
            DrawMode::fill(),
            &mesh_points,
            Color::WHITE
        )
    }

    pub fn get_draw_param(&self) -> DrawParam {
        DrawParam::new()
            .dest(self.pos)
            .rotation(-self.speed.angle_between(Vec2::X))
            .color(self.color)
    }
    pub fn pos(&self) -> Vec2 {
        self.pos
    }
    pub fn speed(&self) -> Vec2 {
        self.speed
    }
    pub fn color(&self) -> Color {
        self.color
    }

    pub fn add_dir(&mut self, direction: Vec2) {
        self.speed += direction;
        // Limit speed:
        const MAX_SPEED: Vec2 = Vec2::new(MAX_BOID_VELOCITY, MAX_BOID_VELOCITY);
        self.speed = self.speed.clamp(-MAX_SPEED, MAX_SPEED);

        if self.speed.length() < MIN_BOID_VELOCITY {
            self.speed = MIN_BOID_VELOCITY * self.speed.normalize_or_zero();
        }
    }
}

// Implementations necessary for being used as hashmap keys:
impl PartialEq for Boid {
    fn eq(&self, other: &Self) -> bool {
        self.pos == other.pos && self.speed == other.speed
    }
}
impl Eq for Boid {}

impl Hash for Boid {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // Convert to OrderedFloat for hashing:
        let (pos_x, pos_y) = (OrderedFloat(self.pos.x), OrderedFloat(self.pos.y));
        let (dir_x, dir_y) = (OrderedFloat(self.speed.x), OrderedFloat(self.speed.y));

        // Hash:
        pos_x.hash(state);
        pos_y.hash(state);
        dir_x.hash(state);
        dir_y.hash(state);
    }
}

// To make distance calculations more efficient, the boids will be located in a grid where each cell
// holds all boids within a certain distance. This struct saves the boid and its location in the
// grid:
#[derive(PartialEq, Eq, Hash)]
pub struct GridBoid {
    // Boid itself:
    pub boid: Boid,

    // Coordinates in grid:
    pub row: usize,
    pub col: usize
}