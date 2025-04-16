# trs - Trash CLI Utility

A command-line utility to safely manage file deletion by using a trash folder instead of permanently deleting files.

## Features

- Move files and directories to trash instead of permanently deleting them
- Restore files from trash to their original locations
- Interactive restoration with file selection
- View contents of trash with original paths
- Permanently empty trash when needed
- Progress indicators for operations
- Preserves directory structures when trashing and restoring

## Installation

### From Source

1. Ensure you have Rust installed ([rustup.rs](https://rustup.rs))
2. Clone the repository:
   ```
   git clone https://github.com/willuhmjs/trs.git
   cd trs
   ```
3. Build and install:
   ```
   cargo install --path .
   ```

### Arch Linux (AUR)

```bash
yay -S trs-git
```

## Usage

### Basic Usage

Move a file to trash:
```bash
trs file.txt
```

Move multiple files to trash:
```bash
trs file1.txt file2.txt directory1
```

### Subcommands

Move files to trash (alternative syntax):
```bash
trs move file1.txt file2.txt directory1
```

Restore files from trash (interactive):
```bash
trs restore
```

Show trash contents:
```bash
trs show
```

Empty trash permanently:
```bash
trs empty
```

### Help

Display help information:
```bash
trs --help
```

## Storage

By default, trash items are stored in your local data directory:
- Linux: `~/.local/share/trash/`
- macOS: `~/Library/Application Support/trash/`
- Windows: `C:\Users\Username\AppData\Local\trash\`

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## Author

William Faircloth
