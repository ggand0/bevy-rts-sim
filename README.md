# Bevy RTS Sim

A real-time battle simulation built with the Bevy game engine in Rust, demonstrating autonomous combat between 10,000 units with basic performance optimizations. Features objective-based gameplay with destructible uplink towers and dramatic sprite sheet explosion effects. The entire codebase was generated with Claude Sonnet 4.

![Screenshot from 2025-06-05 04-36-37](https://github.com/user-attachments/assets/d3c76bfb-d288-4f79-bf01-788e9e7084e8)


## Features

- **Massive Scale Combat**: 10,000 autonomous units (5,000 vs 5,000) with real-time combat simulation
- **Objective-Based Gameplay**: Destroy enemy uplink tower to win, tower destruction triggers cascade explosions
- **Sprite Sheet Explosions**: Custom shader-based 5×5 flipbook animation system with billboard rendering
- **Squad Formation System**: 50-unit squads with tactical formations and commander promotion
- **Autonomous Combat**: Units automatically target, fire, and engage enemies within range
- **Spatial Partitioning**: Grid-based collision optimization reducing complexity from O(n*m) to O(k)
- **RTS Camera Controls**: Smooth WASD movement, mouse rotation, and zoom controls
- **Audio**: 5 random laser sound variations with performance throttling
- **Procedural Generation**: Battle droids and uplink towers generated programmatically

## Getting Started

### Prerequisites

- Rust (latest stable version)
- **Git LFS** (Large File Storage) - for texture and audio assets
- GPU with Vulkan support (recommended for best performance)

### Installation

1. Install Git LFS (if not already installed):
```bash
# Ubuntu/Debian
sudo apt-get install git-lfs

# macOS
brew install git-lfs

# Windows
# Download from https://git-lfs.github.com/
```

2. Clone the repository:
```bash
git clone https://github.com/ggand0/bevy-rts-sim.git
cd bevy-rts-sim
git lfs pull  # Download LFS assets (explosion textures, audio)
```

3. Run the simulation:
```bash
cargo run --release
```

For development with faster compile times:
```bash
cargo run
```

**AMD GPU Users (Linux):** If you experience segfaults on startup, use:
```bash
VK_LOADER_DEBUG=error VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/radeon_icd.x86_64.json cargo run --release
```

## Controls

### Camera
- **WASD**: Camera movement
- **Right-Click + Drag**: Rotate camera view
- **Scroll Wheel**: Zoom in/out

### Combat
- **F**: Volley fire (all units fire simultaneously)

### Formations
- **G**: Advance
- **H**: Retreat

### Debug/Testing
- **E** (during gameplay): Destroy enemy tower (test cascade explosions)
- **Y**: Spawn test animated sprite explosion
- **U**: Spawn test custom shader explosion
- **I**: Spawn test solid color explosion

## Documentation

- **[docs/PROJECT_OVERVIEW.md](docs/PROJECT_OVERVIEW.md)**: Complete architecture and systems reference
- **[docs/EXPLOSION_SYSTEM.md](docs/EXPLOSION_SYSTEM.md)**: Explosion system technical documentation

## Customization

Game parameters can be modified in `src/constants.rs`.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
