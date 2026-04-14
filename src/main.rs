use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Parser)]
#[command(name = "undo")]
#[command(about = "Universal operation undo", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Revert last N operations
    count: Option<usize>,
}

#[derive(Subcommand)]
enum Commands {
    /// Recover last deleted file
    Rm,
    /// Show recent operations
    Ls,
    /// Purge history
    Clear,
    /// Internal: record rm operation
    #[command(hide = true)]
    RecordRm {
        #[arg(required = true)]
        paths: Vec<PathBuf>,
    },
    /// Internal: record mv operation
    #[command(hide = true)]
    RecordMv {
        src: PathBuf,
        dest: PathBuf,
    },
    /// Internal: record cp operation
    #[command(hide = true)]
    RecordCp {
        src: PathBuf,
        dest: PathBuf,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
enum OpType {
    Rm,
    Mv,
    Cp,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct FileRecord {
    original_path: PathBuf,
    trash_id: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct UndoOp {
    id: String,
    timestamp: DateTime<Utc>,
    op_type: OpType,
    files: Vec<FileRecord>,
    // For mv/cp:
    src: Option<PathBuf>,
    dest: Option<PathBuf>,
}

fn get_undo_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow!("Could not find home directory"))?;
    let undo_dir = home.join(".undo");
    if !undo_dir.exists() {
        fs::create_dir_all(&undo_dir)?;
        fs::create_dir_all(undo_dir.join("trash"))?;
    }
    Ok(undo_dir)
}

fn load_history() -> Result<Vec<UndoOp>> {
    let undo_dir = get_undo_dir()?;
    let history_file = undo_dir.join("history.json");
    if !history_file.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(history_file)?;
    let history: Vec<UndoOp> = serde_json::from_str(&content)?;
    Ok(history)
}

fn save_history(history: &[UndoOp]) -> Result<()> {
    let undo_dir = get_undo_dir()?;
    let history_file = undo_dir.join("history.json");
    let content = serde_json::to_string_pretty(history)?;
    fs::write(history_file, content)?;
    Ok(())
}

fn record_rm(paths: &[PathBuf]) -> Result<()> {
    let undo_dir = get_undo_dir()?;
    let trash_dir = undo_dir.join("trash");
    let mut files = Vec::new();

    for path in paths {
        if !path.exists() {
            eprintln!("Warning: {} does not exist, skipping", path.display());
            continue;
        }
        let abs_path = fs::canonicalize(path)?;
        let id = Uuid::new_v4().to_string();
        let trash_path = trash_dir.join(&id);

        // Move to trash
        if abs_path.is_dir() {
            let mut options = fs_extra::dir::CopyOptions::new();
            options.copy_inside = true;
            fs_extra::dir::move_dir(&abs_path, &trash_path, &options)?;
        } else {
            fs::rename(&abs_path, &trash_path)?;
        }

        files.push(FileRecord {
            original_path: abs_path,
            trash_id: id,
        });
    }

    if files.is_empty() {
        return Ok(());
    }

    let op = UndoOp {
        id: Uuid::new_v4().to_string(),
        timestamp: Utc::now(),
        op_type: OpType::Rm,
        files,
        src: None,
        dest: None,
    };

    let mut history = load_history()?;
    history.push(op);
    save_history(&history)?;

    Ok(())
}

fn record_mv(src: PathBuf, dest: PathBuf) -> Result<()> {
    // Before moving, we should check if dest exists and maybe back it up if we want full undo?
    // But usually mv just renames.
    // To undo mv, we just move back.
    
    let abs_src = fs::canonicalize(&src)?;
    // dest might not exist yet if it's the target name
    let abs_dest = if dest.is_dir() {
        dest.join(src.file_name().unwrap())
    } else {
        dest.clone()
    };
    // Note: canonicalize only works if file exists. For dest, we might need to handle it.
    
    // Perform the move
    fs::rename(&src, &dest)?;

    let op = UndoOp {
        id: Uuid::new_v4().to_string(),
        timestamp: Utc::now(),
        op_type: OpType::Mv,
        files: vec![],
        src: Some(abs_src),
        dest: Some(abs_dest),
    };

    let mut history = load_history()?;
    history.push(op);
    save_history(&history)?;
    Ok(())
}

fn record_cp(src: PathBuf, dest: PathBuf) -> Result<()> {
    let abs_src = fs::canonicalize(&src)?;
    
    // Perform the copy
    if src.is_dir() {
        let mut options = fs_extra::dir::CopyOptions::new();
        options.copy_inside = true;
        fs_extra::dir::copy(&src, &dest, &options)?;
    } else {
        fs::copy(&src, &dest)?;
    }

    let abs_dest = if dest.is_dir() {
        dest.join(src.file_name().unwrap())
    } else {
        dest.clone()
    };

    let op = UndoOp {
        id: Uuid::new_v4().to_string(),
        timestamp: Utc::now(),
        op_type: OpType::Cp,
        files: vec![],
        src: Some(abs_src),
        dest: Some(abs_dest),
    };

    let mut history = load_history()?;
    history.push(op);
    save_history(&history)?;
    Ok(())
}

fn undo_op(op: &UndoOp) -> Result<()> {
    let undo_dir = get_undo_dir()?;
    let trash_dir = undo_dir.join("trash");

    match op.op_type {
        OpType::Rm => {
            for file in &op.files {
                let trash_path = trash_dir.join(&file.trash_id);
                if !trash_path.exists() {
                    eprintln!("Warning: trash file {} missing", file.trash_id);
                    continue;
                }
                
                // Ensure parent dir exists
                if let Some(parent) = file.original_path.parent() {
                    fs::create_dir_all(parent)?;
                }

                if trash_path.is_dir() {
                    let mut options = fs_extra::dir::CopyOptions::new();
                    options.copy_inside = true;
                    fs_extra::dir::move_dir(&trash_path, &file.original_path, &options)?;
                } else {
                    fs::rename(&trash_path, &file.original_path)?;
                }
                println!("Recovered: {}", file.original_path.display());
            }
        }
        OpType::Mv => {
            if let (Some(src), Some(dest)) = (&op.src, &op.dest) {
                if dest.exists() {
                    if let Some(parent) = src.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::rename(dest, src)?;
                    println!("Moved {} back to {}", dest.display(), src.display());
                }
            }
        }
        OpType::Cp => {
            if let Some(dest) = &op.dest {
                if dest.exists() {
                    if dest.is_dir() {
                        fs::remove_dir_all(dest)?;
                    } else {
                        fs::remove_file(dest)?;
                    }
                    println!("Removed copy at {}", dest.display());
                }
            }
        }
    }
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Ls) => {
            let history = load_history()?;
            if history.is_empty() {
                println!("No history found.");
                return Ok(());
            }
            for (i, op) in history.iter().rev().enumerate() {
                let time = op.timestamp.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M:%S");
                match op.op_type {
                    OpType::Rm => println!("[{}] {} - RM: {} files", i + 1, time, op.files.len()),
                    OpType::Mv => println!("[{}] {} - MV: {} -> {}", i + 1, time, op.src.as_ref().unwrap().display(), op.dest.as_ref().unwrap().display()),
                    OpType::Cp => println!("[{}] {} - CP: {} -> {}", i + 1, time, op.src.as_ref().unwrap().display(), op.dest.as_ref().unwrap().display()),
                }
            }
        }
        Some(Commands::Clear) => {
            let undo_dir = get_undo_dir()?;
            fs::remove_dir_all(&undo_dir)?;
            println!("History purged.");
        }
        Some(Commands::Rm) => {
            let mut history = load_history()?;
            if let Some(last_rm_idx) = history.iter().rposition(|op| matches!(op.op_type, OpType::Rm)) {
                let op = history.remove(last_rm_idx);
                undo_op(&op)?;
                save_history(&history)?;
            } else {
                println!("No deleted files found in history.");
            }
        }
        Some(Commands::RecordRm { paths }) => {
            record_rm(&paths)?;
        }
        Some(Commands::RecordMv { src, dest }) => {
            record_mv(src, dest)?;
        }
        Some(Commands::RecordCp { src, dest }) => {
            record_cp(src, dest)?;
        }
        None => {
            let count = cli.count.unwrap_or(1);
            let mut history = load_history()?;
            if history.is_empty() {
                println!("No history to undo.");
                return Ok(());
            }
            
            let to_undo = count.min(history.len());
            for _ in 0..to_undo {
                if let Some(op) = history.pop() {
                    undo_op(&op)?;
                }
            }
            save_history(&history)?;
        }
    }

    Ok(())
}
