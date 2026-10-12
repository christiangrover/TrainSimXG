//! Saving and loading layouts (terrain, pieces and trains) as JSON files in `saves/`.

use crate::grid::*;
use crate::trains::{Train, Trains};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct SaveData {
    version: u32,
    size: i32,
    /// Only tiles that differ from empty flat grass, as (index, tile).
    tiles: Vec<(u32, Tile)>,
    trains: Vec<Train>,
    next_train_id: u32,
}

/// `saves/` next to the executable, so a copied game folder keeps its layouts.
pub fn save_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("saves")))
        .unwrap_or_else(|| PathBuf::from("saves"))
}

fn path_for(name: &str) -> PathBuf {
    let clean: String = name.chars().filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_')).collect();
    let clean = clean.trim();
    save_dir().join(format!("{}.json", if clean.is_empty() { "layout" } else { clean }))
}

pub fn list_saves() -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(save_dir())
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .filter_map(|e| {
                    let p = e.path();
                    if p.extension()? == "json" {
                        Some(p.file_stem()?.to_string_lossy().to_string())
                    } else {
                        None
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

pub fn to_json(grid: &Grid, trains: &Trains) -> Result<String, String> {
    let empty = Tile::default();
    let tiles = grid.tiles.iter().enumerate().filter(|(_, t)| **t != empty).map(|(i, t)| (i as u32, *t)).collect();
    let data = SaveData { version: VERSION, size: GRID, tiles, trains: trains.list.clone(), next_train_id: trains.next_id };
    serde_json::to_string(&data).map_err(|e| e.to_string())
}

pub fn save(name: &str, grid: &Grid, trains: &Trains) -> Result<PathBuf, String> {
    let json = to_json(grid, trains)?;
    std::fs::create_dir_all(save_dir()).map_err(|e| e.to_string())?;
    let p = path_for(name);
    std::fs::write(&p, json).map_err(|e| format!("Couldn't write {}: {e}", p.display()))?;
    Ok(p)
}

pub fn load(name: &str, grid: &mut Grid, trains: &mut Trains) -> Result<(), String> {
    let p = path_for(name);
    let json = std::fs::read_to_string(&p).map_err(|e| format!("Couldn't read {}: {e}", p.display()))?;
    from_json(&json, grid, trains)
}

pub fn from_json(json: &str, grid: &mut Grid, trains: &mut Trains) -> Result<(), String> {
    let data: SaveData = serde_json::from_str(json).map_err(|e| format!("That save file looks damaged: {e}"))?;
    if data.size != GRID {
        return Err(format!("Save is for a {0}x{0} table, this one is {GRID}x{GRID}.", data.size));
    }
    let mut tiles = vec![Tile::default(); (GRID * GRID) as usize];
    for (i, t) in data.tiles {
        if let Some(slot) = tiles.get_mut(i as usize) {
            *slot = t;
        }
    }
    grid.replace_all(tiles);
    trains.list = data.trains;
    trains.next_id = data.next_train_id.max(trains.list.iter().map(|t| t.id + 1).max().unwrap_or(0));
    trains.selected = trains.list.first().map(|t| t.id);
    trains.dirty = true;
    Ok(())
}
