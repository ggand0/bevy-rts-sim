# Bevy RTS Sim

A real-time battle simulation built with the Bevy game engine in Rust, demonstrating autonomous combat between 10,000 units with basic performance optimizations. Features spatial partitioning and team-based battles. The entire codebase was generated with Claude Sonnet 4.
![Screenshot from 2025-06-05 04-36-37](https://github.com/user-attachments/assets/d3c76bfb-d288-4f79-bf01-788e9e7084e8)


## Features

- **Rendering Simulation**: 10,000 autonomous units (5,000 vs 5,000) with basic real-time combat simulation
- **Autonomous Combat System**: Units on two teams automatically target, fire, and engage enemies within range
- **Spatial Partitioning**: Grid-based collision optimization reducing complexity from O(n*m) to O(k)
- **RTS Camera Controls**: Smooth WASD movement, mouse rotation, and zoom controls
- **Audio**: 5 random laser sound variations with performance throttling
- **Procedural Units**: Humanoid battle droids generated programmatically

## Getting Started

### Prerequisites

- Rust (latest stable version)
- GPU with Vulkan support (recommended for best performance)

### Installation

1. Clone the repository:
```bash
git clone https://github.com/ggand0/bevy-rts-sim.git
cd bevy-rts-sim
```

2. Run the simulation:
```bash
cargo run --release
```

For development with faster compile times:
```bash
cargo run
```

## Controls

- **WASD**: Camera movement
- **Mouse Drag**: Rotate camera view
- **Scroll Wheel**: Zoom in/out
- **F**: Manual volley fire (all units fire simultaneously)

## Customization
Modify the constants in `main.rs`.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
