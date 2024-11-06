use serde_derive::{Deserialize, Serialize};
use std::{
    error::Error,
    fs::File,
    io::{BufReader, BufWriter},
    ops::{Deref, DerefMut},
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    pub angle_deg: f32,
    pub turn_speed: f32,
}

#[derive(Debug)]
pub struct StateBox {
    pub path: PathBuf,
    pub state: State,
}

impl Default for State {
    fn default() -> Self {
        Self {
            angle_deg: 0.0,
            turn_speed: 0.0,
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
