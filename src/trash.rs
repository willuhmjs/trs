//! Core trash operations implementation

use std::fs;
use std::io::{self, Write, BufRead};
use std::path::Path;
use std::env;
use flate2::write::GzEncoder;
use flate2::Compression;
use flate2::read::GzDecoder;
use std::collections::HashMap;
use tar::{Archive, Builder};
use indicatif::{ProgressBar, ProgressStyle};

use crate::metadata::{load_metadata, save_metadata, TrashItem};

/// Generate a unique filename for the trash by appending a number if necessary
fn generate_unique_name(
    trash_dir: &Path, 
    file_name: &str, 
    original_path: &str, 
    is_directory: bool,
    metadata: &HashMap<String, (String, bool)>
) -> String {
    let file_stem = if file_name.ends_with(".tar.gz") {
        file_name.trim_end_matches(".tar.gz")
    } else if file_name.ends_with(".gz") {
        file_name.trim_end_matches(".gz")
    } else {
        file_name
    };
    
    let original_path = Path::new(original_path);
    let mut unique_name = file_name.to_string();
    let mut counter = 1;
    
    // Check if file with this name already exists in trash and has the same type or comes from a different path
    while trash_dir.join(&unique_name).exists() || 
          metadata.iter().any(|(k, (v, item_is_dir))| {
              k == &unique_name && (*item_is_dir == is_directory || Path::new(v) != original_path)
          }) {
        // If it exists but has the same original path and type, it's not a duplicate
        if metadata.iter().any(|(k, (v, item_is_dir))| {
            k == &unique_name && *item_is_dir == is_directory && Path::new(v) == original_path
        }) {
            break;
        }
        
        // Generate a new numbered name
        if let Some(ext) = Path::new(file_stem).extension() {
            let stem = Path::new(file_stem).file_stem().unwrap().to_string_lossy();
            let ext_str = ext.to_string_lossy();
            unique_name = format!("{}({}){}", stem, counter, if ext_str.is_empty() { "".to_string() } else { format!(".{}", ext_str) });
        } else {
            unique_name = format!("{}({})", file_stem, counter);
        }
        
        // Add back extension if the original had it
        if file_name.ends_with(".tar.gz") {
            unique_name = format!("{}.tar.gz", unique_name);
        } else if file_name.ends_with(".gz") {
            unique_name = format!("{}.gz", unique_name);
        }
        
        counter += 1;
    }
    
    unique_name
}

/// Move a file or directory to trash
pub fn move_to_trash(file: &str, trash_dir: &Path) -> io::Result<()> {
    fs::create_dir_all(trash_dir)?;
    let file_path = Path::new(file);
    
    // Convert to absolute path
    let absolute_path = fs::canonicalize(file_path)?;
    let original_path = absolute_path.to_string_lossy().to_string();
    
    let file_name = file_path.file_name().unwrap().to_string_lossy();
    let metadata_file = trash_dir.join(".metadata");

    // Load existing metadata and convert to new format if needed
    let old_metadata = load_metadata(&metadata_file)?;
    let mut metadata = convert_metadata_if_needed(&old_metadata);
    
    // Check if it's a directory
    let is_directory = file_path.is_dir();
    
    // Generate a unique name for the trash file
    let unique_name = generate_unique_name(trash_dir, &file_name, &original_path, is_directory, &metadata);
    let trash_file = trash_dir.join(&unique_name);

    // Create a progress bar
    let pb = ProgressBar::new(100);
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
        .unwrap()
        .progress_chars("#>-"));
    pb.set_message(format!("Moving {} to Trash", file_name));

    if file_path.is_file() {
        // Update progress
        pb.set_position(10);
        
        // Create a tar.gz archive for individual files
        let trash_file_tar_gz = if !unique_name.ends_with(".tar.gz") { 
            trash_file.with_extension("tar.gz") 
        } else { 
            trash_file
        };

        // Create a tar archive and compress it with gzip
        let tar_gz = fs::File::create(&trash_file_tar_gz)?;
        let enc = GzEncoder::new(tar_gz, Compression::best());
        let mut tar = Builder::new(enc);
        
        pb.set_position(30);
        
        // Add the file to the tar archive, preserving its name
        tar.append_path_with_name(file_path, Path::new(&*file_name))?;
        pb.set_position(70);
        
        tar.finish()?;
        pb.set_position(90);
        
        // Delete the original file after successful archiving
        fs::remove_file(file_path)?;
        
        let display_name = if unique_name == file_name.to_string() { 
            file_name.to_string()
        } else {
            format!("{} (as {})", file_name, unique_name.trim_end_matches(".tar.gz"))
        };
        
        pb.finish_with_message(format!("Moved file {} to Trash", display_name));
        
        // Update metadata with the actual trash name
        let trash_name = trash_file_tar_gz.file_name().unwrap().to_string_lossy().to_string();
        metadata.insert(trash_name, (original_path, false)); // false = file
    } else if is_directory {
        if file_path.read_dir()?.next().is_none() {
            // Empty directory - just move it as is
            pb.set_position(50);
            
            let trash_dir_path = trash_dir.join(&unique_name);
            fs::rename(file_path, &trash_dir_path)?;
            
            pb.finish_with_message(format!("Moved empty directory {} to Trash", file_name));
            
            // Update metadata
            metadata.insert(unique_name, (original_path, true)); // true = directory
        } else {
            // Non-empty directory - create a tar.gz archive
            let trash_file_tar_gz = trash_file.with_extension("tar.gz");
            
            // Create a tar archive and compress it with gzip
            let tar_gz = fs::File::create(&trash_file_tar_gz)?;
            let enc = GzEncoder::new(tar_gz, Compression::best());
            let mut tar = Builder::new(enc);
            
            // Define a base directory path for appending
            let base_path = file_path;
            
            pb.set_position(20);
            
            // Add the directory itself first
            tar.append_dir(file_path.file_name().unwrap(), file_path)?;
            pb.set_position(30);
            
            // Recursive function to add directory contents to tar
            fn add_dir_to_tar(
                tar: &mut Builder<GzEncoder<fs::File>>,
                dir: &Path,
                base_path: &Path,
                pb: &ProgressBar,
            ) -> io::Result<()> {
                for entry in fs::read_dir(dir)? {
                    let entry = entry?;
                    let path = entry.path();
                    
                    // Calculate the relative path from the base directory
                    let rel_path = path.strip_prefix(base_path.parent().unwrap_or(Path::new("")))
                        .unwrap_or(&path);
                    
                    if path.is_file() {
                        tar.append_path_with_name(&path, rel_path)?;
                        pb.inc(1); // Increment progress slightly for each file
                    } else if path.is_dir() {
                        // Create directory entry in the tar
                        tar.append_dir(rel_path, &path)?;
                        
                        // Recursively add subdirectory contents
                        add_dir_to_tar(tar, &path, base_path, pb)?;
                    }
                }
                Ok(())
            }
            
            // Add all contents
            add_dir_to_tar(&mut tar, base_path, base_path, &pb)?;
            
            pb.set_position(80);
            
            // Finalize the archive
            tar.finish()?;
            
            pb.set_position(90);
            
            // Remove the original directory after successful archiving
            fs::remove_dir_all(file_path)?;
            
            let display_name = if unique_name == file_name.to_string() { 
                file_name.to_string()
            } else {
                format!("{} (as {})", file_name, unique_name.trim_end_matches(".tar.gz"))
            };
            
            pb.finish_with_message(format!("Moved directory {} to Trash", display_name));
            
            // Update metadata
            let trash_name = trash_file_tar_gz.file_name().unwrap().to_string_lossy().to_string();
            metadata.insert(trash_name, (original_path, true)); // true = directory
        }
    } else {
        pb.finish_and_clear();
        println!("Failed to move: {} not found", file);
        return Ok(());
    }

    // Save the updated metadata
    save_metadata_with_type(&metadata_file, &metadata)?;
    Ok(())
}

/// Convert old metadata format to new format if needed
fn convert_metadata_if_needed(old_metadata: &HashMap<String, String>) -> HashMap<String, (String, bool)> {
    let mut new_metadata = HashMap::new();
    
    for (key, value) in old_metadata {
        // Check if it's already in the new format
        if value.starts_with("{\"path\":\"") {
            // Try to parse as JSON
            if let Ok(item) = serde_json::from_str::<TrashItem>(value) {
                new_metadata.insert(key.clone(), (item.path, item.is_dir));
                continue;
            }
        }
        
        let is_dir = Path::new(value).exists() && Path::new(value).is_dir();
        new_metadata.insert(key.clone(), (value.clone(), is_dir));
    }
    
    new_metadata
}

/// Save metadata with type information
fn save_metadata_with_type(metadata_file: &Path, metadata: &HashMap<String, (String, bool)>) -> io::Result<()> {
    // Convert to the old format for saving
    let old_format: HashMap<String, String> = metadata
        .iter()
        .map(|(k, (path, is_dir))| {
            let item = TrashItem {
                path: path.clone(),
                is_dir: *is_dir,
            };
            (k.clone(), serde_json::to_string(&item).unwrap_or_else(|_| path.clone()))
        })
        .collect();
    
    save_metadata(metadata_file, &old_format)
}

/// Display contents of trash folder
pub fn show_trash_contents(trash_dir: &Path) -> io::Result<()> {
    let metadata_file = trash_dir.join(".metadata");
    let old_metadata = load_metadata(&metadata_file)?;
    let metadata = convert_metadata_if_needed(&old_metadata);

    if trash_dir.exists() {
        let entries = fs::read_dir(trash_dir)?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().into_string().unwrap_or_default())
            .filter(|name| name != ".metadata") // Exclude metadata file
            .collect::<Vec<_>>();

        if entries.is_empty() {
            println!("Trash is empty.");
        } else {
            println!("{:<5} {:<30} {}", "No.", "Name", "Original Location");
            
            for (i, entry) in entries.iter().enumerate() {
                // Get metadata for this entry
                let (display_name, _, original_location) = get_entry_display_info(trash_dir, entry, &metadata)?;
                
                println!("{:<5} {:<30} {}", i + 1, display_name, original_location);
            }
        }
    } else {
        // Try to create the trs-trash directory
        match fs::create_dir_all(trash_dir) {
            Ok(_) => {
                println!("Trash folder created at: {}", trash_dir.display());
                println!("Trash is empty.");
            },
            Err(e) => {
                println!("Could not create trash folder at {}: {}", trash_dir.display(), e);
            }
        }
    }
    Ok(())
}

/// Get display information for an entry
fn get_entry_display_info(trash_dir: &Path, entry: &str, metadata: &HashMap<String, (String, bool)>) -> io::Result<(String, &'static str, String)> {
    // Check if it's a directory on disk
    let path_is_dir = fs::metadata(trash_dir.join(entry))?.is_dir();
    
    // Get the type and display name
    let is_dir = if let Some((_, is_dir)) = metadata.get(entry)
        .or_else(|| metadata.get(entry.trim_end_matches(".tar.gz")))
        .or_else(|| metadata.get(entry.trim_end_matches(".gz")))
        .or_else(|| metadata.get(&format!("{}.tar.gz", entry.trim_end_matches(".tar.gz"))))
        .or_else(|| metadata.get(&format!("{}.gz", entry.trim_end_matches(".gz")))) {
        *is_dir
    } else {
        path_is_dir
    };
    
    let display_name = if is_dir {
        format!("{}/", entry.trim_end_matches(".tar.gz").trim_end_matches(".gz"))
    } else {
        entry.trim_end_matches(".tar.gz").trim_end_matches(".gz").to_string()
    };
    
    let item_type = if is_dir { "Directory" } else { "File" };
    
    // Get the original location
    let original_location = metadata.get(entry)
        .or_else(|| metadata.get(entry.trim_end_matches(".tar.gz")))
        .or_else(|| metadata.get(entry.trim_end_matches(".gz")))
        .or_else(|| metadata.get(&format!("{}.tar.gz", entry.trim_end_matches(".tar.gz"))))
        .or_else(|| metadata.get(&format!("{}.gz", entry.trim_end_matches(".gz"))))
        .map(|(path, _)| path.as_str())
        .unwrap_or("Unknown");
    
    Ok((display_name, item_type, original_location.to_string()))
}

/// Restore a file from trash
pub fn restore_from_trash(file: &str, trash_dir: &Path) -> io::Result<()> {
    let trash_file = trash_dir.join(file);
    let metadata_file = trash_dir.join(".metadata");
    let old_metadata = load_metadata(&metadata_file)?;
    let mut metadata = convert_metadata_if_needed(&old_metadata);

    // Find the original location and type
    let (original_location, is_dir) = match metadata.get(file) {
        Some((location, is_dir)) => (location.clone(), *is_dir),
        None => {
            // If not found in metadata, create a full path in current directory
            let current_dir = env::current_dir()?.canonicalize()?;
            let path = current_dir.join(file.trim_end_matches(".tar.gz").trim_end_matches(".gz")).to_string_lossy().to_string();
            
            // Check if the trash item is a directory
            let is_dir = trash_file.is_dir();
            (path, is_dir)
        },
    };
    let original_file = Path::new(&original_location);

    // Create a progress bar
    let pb = ProgressBar::new(100);
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
        .unwrap()
        .progress_chars("#>-"));
    pb.set_message(format!("Restoring {} from Trash", file));
    pb.set_position(10);

    // Create parent directories if they don't exist
    if let Some(parent) = original_file.parent() {
        fs::create_dir_all(parent)?;
    }
    pb.set_position(20);

    if trash_file.is_file() {
        let file_stem = file.trim_end_matches(".tar.gz").trim_end_matches(".gz");
        
        // Handle different file types
        if file.ends_with(".tar.gz") {
            // Extract tar.gz archive
            pb.set_message("Reading archive...");
            pb.set_position(30);
            
            let tar_gz = fs::File::open(&trash_file)?;
            let tar = GzDecoder::new(tar_gz);
            let mut archive = Archive::new(tar);
            
            pb.set_message("Extracting files...");
            pb.set_position(50);
            
            // If it's a directory archive, extract to parent directory
            if is_dir {
                // Extract to parent directory
                let parent = original_file.parent().unwrap_or(Path::new("."));
                archive.unpack(parent)?;
                pb.finish_with_message(format!("Restored directory {} from Trash", file_stem));
            } else {
                // For single files, extract just that file to its correct location
                for entry in archive.entries()? {
                    let mut entry = entry?;
                    let _entry_path = entry.path()?;  // Prefix with underscore to indicate intentional non-use
                    
                    // If it's a single file, extract with the correct name
                    entry.unpack(original_file)?;
                    break; // Only extract the first file
                }
                pb.finish_with_message(format!("Restored file {} from Trash", file_stem));
            }
        } else if file.ends_with(".gz") {
            // Handle legacy .gz format for backward compatibility
            pb.set_message("Decompressing file...");
            pb.set_position(40);
            
            let mut decoder = GzDecoder::new(fs::File::open(&trash_file)?);
            let mut restored_content = Vec::new();
            io::copy(&mut decoder, &mut restored_content)?;
            
            pb.set_message("Writing file...");
            pb.set_position(80);
            
            fs::write(original_file, restored_content)?;
            pb.finish_with_message(format!("Restored file {} from Trash", file_stem));
        } else {
            // Just copy the file as is (no compression)
            pb.set_message("Copying file...");
            pb.set_position(50);
            
            fs::copy(&trash_file, original_file)?;
            pb.finish_with_message(format!("Restored file {} from Trash", file_stem));
        }
        
        // Delete the trash file
        pb.set_message("Cleaning up...");
        pb.set_position(90);
        fs::remove_file(trash_file)?;
    } else if trash_file.is_dir() && is_dir {
        // For raw directory (not archived), just move it back
        pb.set_message("Moving directory...");
        pb.set_position(50);
        
        fs::rename(&trash_file, original_file)?;
        pb.finish_with_message(format!("Restored directory {} from Trash", file));
    } else {
        pb.finish_and_clear();
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Failed to restore: {} not found in Trash or type mismatch", file),
        ));
    }

    // Update metadata
    pb.set_message("Updating metadata...");
    pb.set_position(95);
    metadata.remove(file);
    save_metadata_with_type(&metadata_file, &metadata)?;
    pb.finish_and_clear();
    Ok(())
}

/// Empty trash folder permanently
pub fn empty_trash(trash_dir: &Path) -> io::Result<()> {
    if trash_dir.exists() {
        // Create progress bar
        let pb = ProgressBar::new_spinner();
        pb.set_style(ProgressStyle::default_spinner()
            .template("{spinner:.green} {elapsed_precise} {msg}")
            .unwrap());
        pb.set_message("Counting items in Trash...");
        
        // Count the number of entries for better progress indication
        let entry_count = fs::read_dir(trash_dir)?
            .filter_map(|entry| entry.ok())
            .count();
        
        if entry_count > 0 {
            // Switch to a progress bar if there are items to delete
            let pb = ProgressBar::new(entry_count as u64);
            pb.set_style(ProgressStyle::default_bar()
                .template("{spinner:.green} [{elapsed_precise}] [{bar:40.red/yellow}] {pos}/{len} {msg}")
                .unwrap()
                .progress_chars("#>-"));
            pb.set_message("Emptying Trash...");
            
            // Instead of removing the whole directory at once, remove items one by one for progress updates
            for entry_result in fs::read_dir(trash_dir)? {
                if let Ok(entry) = entry_result {
                    let path = entry.path();
                    if path.is_dir() {
                        fs::remove_dir_all(path)?;
                    } else {
                        fs::remove_file(path)?;
                    }
                    pb.inc(1);
                }
            }
            
            pb.finish_with_message("Trash emptied successfully");
        } else {
            pb.finish_with_message("Trash was already empty");
        }
    } else {
        println!("Trash is already empty");
    }
    Ok(())
}

/// Interactive restore from trash
pub fn interactive_restore(trash_dir: &Path) -> io::Result<()> {
    if trash_dir.exists() {
        // Create a spinner while loading trash contents
        let pb = ProgressBar::new_spinner();
        pb.set_style(ProgressStyle::default_spinner()
            .template("{spinner:.green} {elapsed_precise} {msg}")
            .unwrap());
        pb.set_message("Loading trash contents...");
        
        let metadata_file = trash_dir.join(".metadata");
        let old_metadata = load_metadata(&metadata_file)?;
        let metadata = convert_metadata_if_needed(&old_metadata);
        
        let entries = fs::read_dir(trash_dir)?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().into_string().unwrap_or_default())
            .filter(|name| name != ".metadata") // Exclude metadata file
            .collect::<Vec<_>>();

        // Clear the spinner when done
        pb.finish_and_clear();

        if entries.is_empty() {
            println!("Trash is empty.");
            return Ok(());
        }

        println!("Select a file or directory to restore:");
        println!("{:<5} {:<30} {}", "No.", "Name", "Original Location");
        
        for (i, entry) in entries.iter().enumerate() {
            let (display_name, _, original_location) = get_entry_display_info(trash_dir, entry, &metadata)?;
            println!("{:<5} {:<30} {}", i + 1, display_name, original_location);
        }

        print!("Enter the number of the item to restore: ");
        io::stdout().flush()?;

        let stdin = io::stdin();
        let input = stdin.lock().lines().next().unwrap_or_else(|| Ok(String::new()))?;
        if let Ok(choice) = input.trim().parse::<usize>() {
            if choice > 0 && choice <= entries.len() {
                let file_to_restore = &entries[choice - 1];
                restore_from_trash(file_to_restore, trash_dir)?;
            } else {
                println!("Invalid choice.");
            }
        } else {
            println!("Invalid input.");
        }
    } else {
        // Try to create the trs-trash directory
        match fs::create_dir_all(trash_dir) {
            Ok(_) => {
                println!("Trash folder created at: {}", trash_dir.display());
                println!("Trash is empty.");
            },
            Err(e) => {
                println!("Could not create trash folder at {}: {}", trash_dir.display(), e);
            }
        }
    }
    Ok(())
}
