use crate::boid::{Boid, GridBoid};
use crate::constants::*;
use ggez::conf::{WindowMode, WindowSetup};
use ggez::event::{EventHandler, MouseButton};
use ggez::glam::Vec2;
use ggez::graphics::{Canvas, Color, DrawMode, DrawParam, InstanceArray, Mesh};
use ggez::input::keyboard::KeyInput;
use ggez::winit::event::VirtualKeyCode;
use ggez::{Context, ContextBuilder, GameError};
use rand::Rng;
use std::collections::{HashMap, HashSet};

mod boid;
mod constants;

fn main() {
    // Initialize window:
    let (context, event_loop) = ContextBuilder::new(PROGRAM_NAME, AUTHOR)
        .window_mode(
            WindowMode::default()
                .dimensions(SCREEN_WIDTH, SCREEN_HEIGHT)
                .max_dimensions(SCREEN_WIDTH, SCREEN_HEIGHT)
                .min_dimensions(SCREEN_WIDTH, SCREEN_HEIGHT)
                .resizable(false),
        )
        .window_setup(WindowSetup::default().title(PROGRAM_NAME))
        .build()
        .expect("Couldn't initialize window");

    // Initialize simulation:
    let sim = BoidsSim::new();

    // Run simulation:
    ggez::event::run(context, event_loop, sim);
}

fn randf(a: f32, b: f32) -> f32 {
    let mut rand = rand::thread_rng();
    rand.gen_range(a..b)
}

/// Runs the given function for all adjacent cells in the grid AND THE CURRENT CELL.
fn run_for_neighbor_cells<F>(row: usize, col: usize, width: usize, height: usize, mut f: F)
where
    F: FnMut(usize, usize),
{
    for row_shift in -1isize..=1isize {
        // Validate row:
        let current_row = row_shift + row as isize;
        if current_row >= 0 && current_row < height as isize {
            for col_shift in -1isize..=1isize {
                // Validate column:
                let current_col = col_shift + col as isize;
                if current_col >= 0 && current_col < width as isize {
                    // Fun method:
                    f(current_row as usize, current_col as usize);
                }
            }
        }
    }
}

struct BoidsSim {
    // The grid divides the screen into cells, and each cell contains a list of the boids in it. The
    // grid only saves indices to the 'boids' vector (to avoid references):
    location_grid: Vec<Vec<HashSet<usize>>>,

    // All boids in the simulation and their indices in the location grid:
    boids: Vec<GridBoid>,

    // A location all boids will aim towards:
    target: Option<Vec2>,

    // Whether boids should avoid walls or not:
    restrict_walls: bool,

    // Index of the leader boid:
    leader_idx: Option<usize>,
}

impl BoidsSim {
    pub fn new() -> Self {
        let (location_grid, boids) = Self::get_random_boids();

        Self {
            location_grid,
            boids,
            target: None,
            restrict_walls: true,
            leader_idx: None,
        }
    }

    fn get_random_boids() -> (Vec<Vec<HashSet<usize>>>, Vec<GridBoid>) {
        // Create the location grid:
        let mut location_grid =
            vec![vec![HashSet::new(); LOCATION_GRID_WIDTH]; LOCATION_GRID_HEIGHT];

        // Create boids (position them at the center of each location cell):
        let boids: Vec<GridBoid> = (0..BOIDS_NUM)
            .map(|i| {
                // Create boid with no particular color:
                let boid = Boid::new(
                    randf(MARGIN, SCREEN_WIDTH - MARGIN),
                    randf(MARGIN, SCREEN_HEIGHT - MARGIN),
                    BOID_COLORS[i % BOID_COLORS.len()],
                );

                // Calculate row and column:
                let (col, row) = (
                    (boid.pos().x / INFLUENCE_DISTANCE) as usize,
                    (boid.pos().y / INFLUENCE_DISTANCE) as usize,
                );

                // Change add index to location grid:
                location_grid[row][col].insert(i);

                // Return GridBoid:
                GridBoid { boid, row, col }
            })
            .collect();

        (location_grid, boids)
    }

    fn update_boids(&mut self) {
        // Recalculate indices:
        self.recalculate_boid_indices();

        // Update directions:
        self.update_boids_directions();

        // Move boids:
        for grid_boid in self.boids.iter_mut() {
            grid_boid.boid.go_forward();
        }
    }

    fn update_boids_directions(&mut self) {
        // Calculate new directions for each boid based on these rules:
        // 1) Don't go towards other boids (Separation).
        // 2) Align direction with close boids' direction (Alignment).
        // 3) Go towards the average location of close boids (Cohesion).
        // 4) Avoid screen walls (Evasion).
        // Calculate each rule in a different thread.
        let directions_matrix = crossbeam::thread::scope(|s| {
            let sep_thread = s.spawn(|_| self.calc_separation_directions());
            let align_thread = s.spawn(|_| self.calc_alignment_directions());
            let coh_thread = s.spawn(|_| self.calc_cohesion_directions());
            let eva_thread = s.spawn(|_| {
                if self.restrict_walls {
                    self.calc_evasion_directions()
                } else {
                    (0..self.boids.len()).map(|_| Vec2::ZERO).collect()
                }
            });
            let target_thread = s.spawn(|_| self.calc_target_directions());
            let leader_thread = s.spawn(|_| self.calc_leader_directions());

            // Join all threads and put in a vector:
            vec![
                sep_thread.join().expect("Error in separation thread"),
                align_thread.join().expect("Error in separation thread"),
                coh_thread.join().expect("Error in separation thread"),
                eva_thread.join().expect("Error in separation thread"),
                target_thread.join().expect("Error in target thread"),
                leader_thread.join().expect("Error in leader thread"),
            ]
        })
        .expect("Error creating threads");

        // Each row in the matrix is a different rule, combine them to a vector with size boids.len:
        let directions_vector: Vec<Vec2> = (0..self.boids.len())
            .map(move |i| {
                let mut sum = Vec2::ZERO;
                for rule_idx in 0..directions_matrix.len() {
                    sum += directions_matrix[rule_idx][i];
                }
                sum
            })
            .collect();

        // For each boid, add directions:
        for (i, direction) in directions_vector.into_iter().enumerate() {
            self.boids[i].boid.add_dir(direction);
        }
    }

    /// Calculates a vector of length `self.boids.len()` of directions towards the target. Each
    /// direction corresponds to a single boid in the `self.boids` vector.
    /// If a target is not specified, all directions are `Vec2::Zero`.
    fn calc_target_directions(&self) -> Vec<Vec2> {
        // If there is a target, move the boids towards it:
        if let Some(target_pos) = self.target {
            (0..self.boids.len())
                .map(|i| TARGET_FACTOR * (target_pos - self.boids[i].boid.pos()))
                .collect()
        } else {
            vec![Vec2::ZERO; self.boids.len()]
        }
    }

    /// Calculates a vector of length `self.boids.len()` of directions towards the leader. Each
    /// direction corresponds to a single boid in the `self.boids` vector.
    /// If a leader is not specified, all directions are `Vec2::Zero`.
    fn calc_leader_directions(&self) -> Vec<Vec2> {
        // If there is a leader , move the boids towards it:
        if let Some(idx) = self.leader_idx {
            (0..self.boids.len())
                .map(|i| LEADER_FACTOR * (self.boids[idx].boid.pos() - self.boids[i].boid.pos()))
                .collect()
        } else {
            vec![Vec2::ZERO; self.boids.len()]
        }
    }

    /// According to boids' rule of separation, returns a vector containing the directions that
    /// point away from nearby boids.
    /// Each direction in the returned vector maps to the boid in the same index in the `boids`
    /// vector.
    fn calc_separation_directions(&self) -> Vec<Vec2> {
        // Create a cache for storing results of vector subtraction (saves half of computations
        // because after calculating a - b we don't need to calculate b - a):
        let mut sub_cache: HashMap<(&GridBoid, &GridBoid), Vec2> = HashMap::new();

        // Create the vector:
        self.boids
            .iter()
            .enumerate()
            .map(|(i, this)| {
                // Initial direction vector:
                let mut dir = Vec2::ZERO;

                // For each adjacent cell and the current one:
                run_for_neighbor_cells(
                    this.row,
                    this.col,
                    LOCATION_GRID_WIDTH,
                    LOCATION_GRID_HEIGHT,
                    |row, col| {
                        // Loop over all boids in the cell:
                        for &other_idx in self.location_grid[row][col].iter() {
                            // Avoid current boid:
                            if i == other_idx {
                                continue;
                            }
                            // Check that the distance between boids is within the influence radius:
                            let other = &self.boids[other_idx];
                            if this.boid.pos().distance_squared(other.boid.pos())
                                > STEERING_DISTANCE_SQUARED
                            {
                                continue;
                            }

                            // Check if the calculation is saved in the sub cache:
                            if let Some(&sub) = sub_cache.get(&(other, this)) {
                                // Remember that saved calculation is this - other and we need other - this:
                                dir -= sub;
                            }
                            // If not calculate it and save in cache:
                            else {
                                let sub = other.boid.pos() - this.boid.pos();
                                sub_cache.insert((this, other), sub);
                                dir += sub;
                            }
                        }
                    },
                );
                // Don't forget to invert and multiply by factor:
                -SEPARATION_FACTOR * dir
            })
            .collect()
    }

    /// According to boids' rule of alignment, returns a vector containing the difference between
    /// each boid's current direction and the average direction of boids close to it who share its
    /// color.
    /// Each direction in the returned vector maps to the boid in the same index in the `boids`
    /// vector.
    fn calc_alignment_directions(&self) -> Vec<Vec2> {
        self.boids
            .iter()
            .map(|this| {
                // Initialize sum and counter:
                let mut sum = Vec2::ZERO;
                let mut count = 0usize;

                // Calculate the average direction of nearby boids:
                run_for_neighbor_cells(
                    this.row,
                    this.col,
                    LOCATION_GRID_WIDTH,
                    LOCATION_GRID_HEIGHT,
                    |row, col| {
                        for other_idx in &self.location_grid[row][col] {
                            // Check that the distance between boids is within the influence radius:
                            let other = &self.boids[*other_idx];
                            if this.boid.pos().distance_squared(other.boid.pos())
                                > INFLUENCE_DISTANCE_SQUARED
                            {
                                continue;
                            }
                            // Check if they have different colors:
                            else if this.boid.color() != other.boid.color() {
                                continue;
                            }

                            // Add current direction to average (this includes our direction):
                            sum += other.boid.speed();
                            count += 1;
                        }
                    },
                );
                // If there are no close boids, return 0:
                if count == 1 {
                    return Vec2::ZERO;
                }
                // Return the difference between the average direction and the boid's direction:
                ALIGNMENT_FACTOR * (sum * (count as f32).recip() - this.boid.speed())
            })
            .collect()
    }

    /// According to boids' rule of cohesion, returns a vector containing the difference between
    /// each boid's current position and the average position of close boids who share its color.
    /// Each direction in the returned vector maps to the boid in the same index in the `boids`
    /// vector.
    fn calc_cohesion_directions(&self) -> Vec<Vec2> {
        self.boids
            .iter()
            .map(|this| {
                // Initialize sum and counter:
                let mut sum = Vec2::ZERO;
                let mut count = 0usize;

                // Calculate the average direction of nearby boids:
                run_for_neighbor_cells(
                    this.row,
                    this.col,
                    LOCATION_GRID_WIDTH,
                    LOCATION_GRID_HEIGHT,
                    |row, col| {
                        for other_idx in &self.location_grid[row][col] {
                            // Check that the distance between boids is within the influence radius:
                            let other = &self.boids[*other_idx];
                            if this.boid.pos().distance_squared(other.boid.pos())
                                > INFLUENCE_DISTANCE_SQUARED
                            {
                                continue;
                            }
                            // Check if they have different colors:
                            else if this.boid.color() != other.boid.color() {
                                continue;
                            }

                            // Add current position to average (this includes our position):
                            sum += other.boid.pos();
                            count += 1;
                        }
                    },
                );

                // If there are no close boids, return 0:
                if count == 1 {
                    return Vec2::ZERO;
                }

                // Return the difference between the average position and the boid's position:
                COHESION_FACTOR * (sum * (count as f32).recip() - this.boid.pos())
            })
            .collect()
    }

    /// According to boids' rule of evasion, returns a vector of directions that avoid obstacles.
    /// Obstacles are currently just walls, but may be more later.
    /// Each direction in the returned vector maps to the boid in the same index in the `boids`
    /// vector.
    fn calc_evasion_directions(&self) -> Vec<Vec2> {
        self.boids
            .iter()
            .map(|grid_boid| {
                // Initialize vector with no evasion:
                let mut dir = Vec2::ZERO;

                // Check floor and ceiling:
                let pos = grid_boid.boid.pos();
                if pos.y < MARGIN {
                    dir.y = EVASION_FACTOR; // Go down
                } else if pos.y > SCREEN_HEIGHT - MARGIN {
                    dir.y = -EVASION_FACTOR; // Go up
                }

                // Check two walls:
                if pos.x < MARGIN {
                    dir.x = EVASION_FACTOR; // Go right
                } else if pos.x > SCREEN_WIDTH - MARGIN {
                    dir.x = -EVASION_FACTOR; // Go left
                }

                // Return final direction:
                dir
            })
            .collect()
    }

    /// Recalculates the indices of the boids inside the grid.
    fn recalculate_boid_indices(&mut self) {
        // For each boid:
        self.boids
            .iter_mut()
            .enumerate()
            .for_each(|(i, grid_boid)| {
                // Calculate new indices:
                let pos = grid_boid.boid.pos();
                let (row, col) = (
                    (pos.y / INFLUENCE_DISTANCE) as usize,
                    (pos.x / INFLUENCE_DISTANCE) as usize,
                );

                // Remove the current index from the outdated grid cell:
                self.location_grid[grid_boid.row][grid_boid.col].remove(&i);

                // Update in boid:
                (grid_boid.row, grid_boid.col) = (row, col);

                // Update in location grid:
                self.location_grid[row][col].insert(i);
            });
    }
}

impl EventHandler for BoidsSim {
    fn update(&mut self, ctx: &mut Context) -> Result<(), GameError> {
        // Calculate change in time per frame:
        while ctx.time.check_update_time(FPS) {
            // Update boids:
            self.update_boids();
        }

        Ok(())
    }

    fn draw(&mut self, ctx: &mut Context) -> Result<(), GameError> {
        // Get canvas:
        let mut canvas = Canvas::from_frame(ctx, Color::WHITE);

        // Create new instance array with boids' drawing parameters:
        let mut draw_params_arr = InstanceArray::new(ctx, None);
        let draw_params: Vec<DrawParam> = self
            .boids
            .iter()
            .map(|grid_boid| grid_boid.boid.get_draw_param())
            .collect();
        draw_params_arr.set(draw_params);

        // Draw a circle around the leader:
        if let Some(idx) = self.leader_idx {
            canvas.draw(
                &Mesh::new_circle(
                    ctx,
                    DrawMode::stroke(5.),
                    self.boids[idx].boid.pos(),
                    30.,
                    1.,
                    Color::YELLOW,
                )?,
                DrawParam::default(),
            );
        }

        // Draw the boids' mesh with the drawing parameters:
        canvas.draw_instanced_mesh(
            Boid::get_boid_mesh(ctx)?,
            &draw_params_arr,
            DrawParam::default(),
        );
        // Draw the target:
        if let Some(target_pos) = self.target {
            let target_circle =
                Mesh::new_circle(ctx, DrawMode::fill(), target_pos, 10., 1., Color::RED)?;
            canvas.draw(&target_circle, DrawParam::default());
        }

        // Finish the canvas:
        canvas.finish(ctx)
    }

    fn mouse_button_down_event(
        &mut self,
        _ctx: &mut Context,
        _button: MouseButton,
        x: f32,
        y: f32,
    ) -> Result<(), GameError> {
        // Set the target as the pressed location:
        self.target = Some(Vec2::new(x, y));

        Ok(())
    }

    fn key_down_event(
        &mut self,
        _ctx: &mut Context,
        input: KeyInput,
        _repeated: bool,
    ) -> Result<(), GameError> {
        if let Some(keycode) = input.keycode {
            match keycode {
                // If the user pressed space, delete target:
                VirtualKeyCode::Space => {
                    self.target = None;
                }
                // If the user pressed w, toggle walls:
                VirtualKeyCode::W => {
                    self.restrict_walls = !self.restrict_walls;
                }
                // If the user pressed l, toggle leader index:
                VirtualKeyCode::L => {
                    if let Some(_) = self.leader_idx {
                        self.leader_idx = None;
                    } else {
                        self.leader_idx = Some(0);
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }
}
