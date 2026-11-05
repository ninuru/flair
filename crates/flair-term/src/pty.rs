use std::{fs::File, io, path::PathBuf};

use flair_core::geom::GridSize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: Vec<(String, String)>,
    pub size: GridSize,
}

pub struct Pty {}

impl Pty {
    #[allow(unused_variables)]
    pub fn spawn(spec: &SpawnSpec) -> io::Result<Pty> {
        todo!()
    }

    pub fn reader(&self) -> io::Result<File> {
        todo!()
    }

    pub fn writer(&self) -> io::Result<File> {
        todo!()
    }

    #[allow(unused_variables)]
    pub fn resize(&mut self, size: GridSize) {
        todo!()
    }

    pub fn pid(&self) -> u32 {
        todo!()
    }

    pub fn kill(&mut self) {
        todo!()
    }
}
