use serde_derive::{Deserialize, Serialize};
use std::{
    error::Error,
    fs::File,
    io::{BufReader, BufWriter},
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
};

use crate::{
    math::{vec2, vec3, vec4, Vector},
    MAX_PARTICLE_COUNT,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    pub orbit_center: vec3,
    pub orbit_distance: vec2,
    pub angle_deg: f32,
    pub turn_speed: f32,

    pub particle_count: u32,
    pub init_ttl: f32,
    pub init_pos: vec4,
    pub init_vel: vec4,
    pub accel: vec4,
}

#[derive(Debug)]
pub struct StateBox {
    pub path: PathBuf,
    pub state: State,
}

impl Default for State {
    fn default() -> Self {
        Self {
            orbit_center: Vector([0.0; 3]),
            orbit_distance: Vector([1.0; 2]),
            angle_deg: 0.0,
            turn_speed: 0.0,

            particle_count: MAX_PARTICLE_COUNT as _,
            init_ttl: 0.0,
            init_pos: Vector([0.0; 4]),
            init_vel: Vector([0.0; 4]),
            accel: Vector([0.0; 4]),
        }
    }
}

impl State {
    pub fn update(&mut self, dt_nanos: u64) {
        let angle_deg_delta = 1e-9 * dt_nanos as f32;
        self.angle_deg += self.turn_speed * angle_deg_delta;
        while self.angle_deg > 180.0 {
            self.angle_deg -= 360.0;
        }
        while self.angle_deg < -180.0 {
            self.angle_deg += 360.0;
        }
    }

    pub fn try_save(&self, path: impl AsRef<Path>) -> Result<(), Box<dyn Error>> {
        let file = File::create(path)?;
        let writer = BufWriter::new(file);
        Ok(serde_json::to_writer_pretty(writer, self)?)
    }

    pub fn try_load(path: impl AsRef<Path>) -> Result<Self, Box<dyn Error>> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        Ok(serde_json::from_reader(reader)?)
    }
}

impl StateBox {
    pub fn load(path: PathBuf) -> Self {
        let state = State::try_load(&path).unwrap_or_default();
        Self { path, state }
    }
}

impl Drop for StateBox {
    fn drop(&mut self) {
        match self.state.try_save(&self.path) {
            Ok(()) => {}
            Err(err) => eprintln!("{err}"),
        }
    }
}

impl Deref for StateBox {
    type Target = State;
    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl DerefMut for StateBox {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.state
    }
}
