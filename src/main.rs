use ncurses::*;
use rand::{rngs::ThreadRng, RngExt};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy)]
struct Vec2f {
    x: f64,
    y: f64,
}

impl Vec2f {
    pub fn new(x: f64, y: f64) -> Self {
        Self {
            x,
            y,
        }
    }
}

const INITIAL_SPEED: f64 = 80.0;
const GRAVITY: f64 = 200.0;
const AIR_DRAG_PER_SECOND: f64 = 0.12;
const PARTICLE_LIFETIME: f64 = 4.0;
const SPAWN_RATE_PER_SECOND: f64 = 30.0;
const BOUNCE_DAMPING: f64 = 0.75;
const MAX_DT: f64 = 0.05;
const MAX_PARTICLES: usize = 4_000;
const BURST_COUNT: usize = 35;
const FRAME_SLEEP_MS: u64 = 8;
const HORIZONTAL_FLOOR_FRICTION: f64 = 0.96;
const TRAIL_STEPS: usize = 5;
const EMITTER_STEP: f64 = 1.5;

#[derive(Debug, Clone, Copy)]
struct Particle {
    pos: Vec2f,
    vel: Vec2f,
    life: f64,
    trail: [Vec2f; TRAIL_STEPS],
}

impl Particle {
    pub fn generate(emitter: Vec2f, rng: &mut ThreadRng) -> Self {
        let spawn_pos = Vec2f::new(
            emitter.x + rng.random_range(-1.0..=1.0),
            emitter.y + rng.random_range(-0.5..=0.5),
        );

        let mut particle = Particle {
            pos: spawn_pos,
            vel: Vec2f::new(0.0, 0.0),
            life: PARTICLE_LIFETIME,
            trail: [spawn_pos; TRAIL_STEPS],
        };

        let angle: f64 = -std::f64::consts::FRAC_PI_2 + rng.random_range(-0.8..=0.8);
        let speed: f64 = INITIAL_SPEED * rng.random_range(0.6..=1.2);
        let vx = speed * angle.cos();
        let vy = speed * angle.sin(); 
        particle.vel = Vec2f::new(vx, vy);
        
        particle
    }

    pub fn apply_velocity(&mut self, dt: f64) {
        for idx in (1..TRAIL_STEPS).rev() {
            self.trail[idx] = self.trail[idx - 1];
        }
        self.trail[0] = self.pos;

        let drag = (1.0 - AIR_DRAG_PER_SECOND * dt).max(0.0);
        self.vel.x *= drag;
        self.vel.y *= drag;
        self.vel.y += GRAVITY * dt;
        self.pos.x += self.vel.x * dt;
        self.pos.y += self.vel.y * dt;
    }

    fn draw_point(x: i32, y: i32, glyph: char, pair: i16, use_colors: bool) {
        if use_colors {
            attron(COLOR_PAIR(pair));
        }
        mvaddch(y, x, glyph as u32);
        if use_colors {
            attroff(COLOR_PAIR(pair));
        }
    }

    pub fn display(&self, width: i32, height: i32, use_colors: bool) {
        let max_x = (width - 2).max(1);
        let max_y = (height - 2).max(1);
        let ix = self.pos.x.round() as i32;
        let iy = self.pos.y.round() as i32;
        if ix < 1 || ix > max_x || iy < 1 || iy > max_y {
            return;
        }

        let life_ratio = (self.life / PARTICLE_LIFETIME).clamp(0.0, 1.0);
        let (pair, glyph) = if life_ratio > 0.66 {
            (YELLOW_PAIR, '*')
        } else if life_ratio > 0.33 {
            (RED_PAIR, '+')
        } else {
            (BLUE_PAIR, '.')
        };

        let trail_glyphs: [char; TRAIL_STEPS] = [';', ':', '.', '.', '.'];
        for idx in (0..TRAIL_STEPS).rev() {
            let tx = self.trail[idx].x.round() as i32;
            let ty = self.trail[idx].y.round() as i32;
            if tx < 1 || tx > max_x || ty < 1 || ty > max_y {
                continue;
            }
            let trail_pair = if idx < 2 { RED_PAIR } else { BLUE_PAIR };
            Self::draw_point(tx, ty, trail_glyphs[idx], trail_pair, use_colors);
        }

        Self::draw_point(ix, iy, glyph, pair, use_colors);
    }

    pub fn bounce_off_walls(&mut self, width: i32, height: i32) {
        let min_x = 1.0;
        let max_x = (width - 2) as f64;
        let min_y = 1.0;
        let max_y = (height - 2) as f64;

        if self.pos.x <= min_x || self.pos.x >= max_x {
            self.pos.x = self.pos.x.clamp(min_x, max_x);
            self.vel.x *= -BOUNCE_DAMPING;
        }

        if self.pos.y <= min_y || self.pos.y >= max_y {
            self.pos.y = self.pos.y.clamp(min_y, max_y);
            self.vel.y *= -BOUNCE_DAMPING;

            if (self.pos.y - max_y).abs() < f64::EPSILON {
                self.vel.x *= HORIZONTAL_FLOOR_FRICTION;
            }
        }
    }
}

struct SparkSimulation {
    particles: Vec<Particle>,
    spawn_carry: f64,
    use_colors: bool,
    emitter: Option<Vec2f>,
}

impl SparkSimulation {
    pub fn new(use_colors: bool) -> Self {
        Self {
            particles: vec![],
            spawn_carry: 0.0,
            use_colors,
            emitter: None,
        }
    }

    pub fn run(&mut self, rng: &mut ThreadRng) {
        let mut last_tick = Instant::now();
        
        loop {
            let now = Instant::now();
            let mut dt = (now - last_tick).as_secs_f64();
            last_tick = now;
            dt = dt.clamp(0.0, MAX_DT);

            let mut width: i32 = -1;
            let mut height: i32 = -1;
            getmaxyx(stdscr(), &mut height, &mut width);

            if width < 3 || height < 3 {
                erase();
                let _ = mvprintw(0, 0, "Terminal too small. Resize to continue.");
                refresh();
                std::thread::sleep(Duration::from_millis(FRAME_SLEEP_MS));
                continue;
            }

            if self.emitter.is_none() {
                self.emitter = Some(Vec2f::new((width as f64) / 2.0, (height as f64) - 3.0));
            }
            self.clamp_emitter(width, height);
            let emitter = self
                .emitter
                .expect("emitter must be initialized before simulation step");

            self.spawn_particles(dt, emitter, rng);
            self.step_particles(dt, width, height);
            self.cull_particles();
            self.display_particles(width, height, emitter);

            if !self.handle_input(width, height, rng) {
                return;
            }

            std::thread::sleep(Duration::from_millis(FRAME_SLEEP_MS));
        }
    }

    fn spawn_particle(&mut self, emitter: Vec2f, rng: &mut ThreadRng) {
        if self.particles.len() < MAX_PARTICLES {
            self.particles.push(Particle::generate(emitter, rng));
        }
    }

    fn spawn_particles(&mut self, dt: f64, emitter: Vec2f, rng: &mut ThreadRng) {
        let wanted = self.spawn_carry + SPAWN_RATE_PER_SECOND * dt;
        let spawn_count = wanted.floor() as usize;
        self.spawn_carry = wanted - spawn_count as f64;

        for _ in 0..spawn_count {
            self.spawn_particle(emitter, rng);
        }
    }

    fn spawn_burst(&mut self, count: usize, emitter: Vec2f, rng: &mut ThreadRng) {
        for _ in 0..count {
            self.spawn_particle(emitter, rng);
        }
    }

    fn clamp_emitter(&mut self, width: i32, height: i32) {
        if let Some(mut emitter) = self.emitter {
            emitter.x = emitter.x.clamp(1.0, (width - 2) as f64);
            emitter.y = emitter.y.clamp(1.0, (height - 2) as f64);
            self.emitter = Some(emitter);
        }
    }

    fn move_emitter(&mut self, dx: f64, dy: f64, width: i32, height: i32) {
        let current = self
            .emitter
            .unwrap_or(Vec2f::new((width as f64) / 2.0, (height as f64) - 3.0));
        self.emitter = Some(Vec2f::new(current.x + dx, current.y + dy));
        self.clamp_emitter(width, height);
    }

    fn handle_input(&mut self, width: i32, height: i32, rng: &mut ThreadRng) -> bool {
        loop {
            let ch = getch();
            if ch == ERR {
                break;
            }

            match ch {
                113 | 81 => return false,
                114 | 82 => self.particles.clear(),
                32 => {
                    if let Some(emitter) = self.emitter {
                        self.spawn_burst(BURST_COUNT, emitter, rng);
                    }
                }
                KEY_LEFT | 97 | 65 => self.move_emitter(-EMITTER_STEP, 0.0, width, height),
                KEY_RIGHT | 100 | 68 => self.move_emitter(EMITTER_STEP, 0.0, width, height),
                KEY_UP | 119 | 87 => self.move_emitter(0.0, -EMITTER_STEP, width, height),
                KEY_DOWN | 115 | 83 => self.move_emitter(0.0, EMITTER_STEP, width, height),
                _ => {}
            }
        }

        true
    }
    
    fn display_particles(&self, width: i32, height: i32, emitter: Vec2f) {
        erase();
        box_(stdscr(), 0, 0);

        for particle in &self.particles {
            particle.display(width, height, self.use_colors);
        }

        mvaddch(emitter.y as i32, emitter.x as i32, '^' as u32);

        let hud = format!(
            "particles: {} | [Q] quit [R] clear [Space] burst [Arrows/WASD] move source",
            self.particles.len()
        );
        let max_chars = (width - 2).max(0) as usize;
        let clipped_hud: String = hud.chars().take(max_chars).collect();
        let _ = mvprintw(height - 1, 1, &clipped_hud);

        refresh();
    }

    fn step_particles(&mut self, dt: f64, width: i32, height: i32) {
        for particle in &mut self.particles {
            particle.apply_velocity(dt);
            particle.bounce_off_walls(width, height);
            particle.life -= dt;
        }
    }

    fn cull_particles(&mut self) {
        self.particles.retain(|p| p.life > 0.0);
    }
}

const YELLOW_PAIR: i16 = 1;
const BLUE_PAIR: i16 = 2;
const RED_PAIR: i16 = 3;

fn init_colors() -> bool {
    if has_colors() {
        start_color();
        use_default_colors();

        init_pair(YELLOW_PAIR, COLOR_YELLOW, -1);
        init_pair(BLUE_PAIR, COLOR_BLUE, -1);
        init_pair(RED_PAIR, COLOR_RED, -1);
        true
    } else {
        false
    }
}

struct NcursesGuard;

impl Drop for NcursesGuard {
    fn drop(&mut self) {
        endwin();
    }
}

fn main() {
    initscr();
    let _guard = NcursesGuard;

    cbreak();
    noecho();
    keypad(stdscr(), true);
    nodelay(stdscr(), true);
    let _ = curs_set(CURSOR_VISIBILITY::CURSOR_INVISIBLE);
    let use_colors = init_colors();

    let mut rng = rand::rng();

    let mut sim = SparkSimulation::new(use_colors);
    sim.run(&mut rng);
}
