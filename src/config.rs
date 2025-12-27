use crate::utils::Vector2D;
use clap::{Parser, ValueEnum};

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum MoveType {
    Corner,
    Cursor,
}

impl Default for MoveType {
    fn default() -> Self {
        Self::Cursor
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub move_type: MoveType,
    pub size: Vector2D,
    pub render_inactive: bool,
    pub no_fractional: bool,
    pub continuous_capture: bool,
    pub zoom_speed: f64,
    pub exit_delay_ms: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            move_type: MoveType::Cursor,
            size: Vector2D::new(300.0, 150.0),
            render_inactive: false,
            no_fractional: false,
            continuous_capture: true,
            zoom_speed: 0.05, // Default zoom speed (5% per scroll notch)
            exit_delay_ms: 200, // Default 200ms delay before exit
        }
    }
}

#[derive(Parser)]
#[command(name = "remagnify")]
#[command(about = "A wlroots-compatible Wayland magnifier", long_about = None)]
#[command(version)]
pub struct Cli {
    /// Magnifier move type
    #[arg(short = 'm', long, value_enum, default_value = "cursor")]
    pub move_type: MoveType,

    /// Size of magnifier (WIDTHxHEIGHT)
    #[arg(short, long, value_parser = parse_size)]
    pub size: Option<Vector2D>,

    /// Render (freeze) inactive displays
    #[arg(short, long)]
    pub render_inactive: bool,

    /// Disable fractional scaling
    #[arg(short = 't', long)]
    pub no_fractional: bool,

    /// Enable continuous capture (live updates)
    #[arg(short, long, default_value = "true")]
    pub continuous: bool,

    /// Zoom speed multiplier (default: 0.05, higher = faster)
    #[arg(short = 'z', long, default_value = "0.05")]
    pub zoom_speed: f64,

    /// Exit delay in milliseconds after zooming out (default: 200)
    #[arg(short = 'e', long, default_value = "200")]
    pub exit_delay: u64,

    /// Quiet mode
    #[arg(short, long)]
    pub quiet: bool,

    /// Verbose logging
    #[arg(short, long)]
    pub verbose: bool,
}

fn parse_size(s: &str) -> Result<Vector2D, String> {
    let parts: Vec<&str> = s.split('x').collect();
    if parts.len() != 2 {
        return Err(format!("Size must be in format WIDTHxHEIGHT, got: {}", s));
    }

    let width: f64 = parts[0]
        .parse()
        .map_err(|_| format!("Invalid width: {}", parts[0]))?;
    let height: f64 = parts[1]
        .parse()
        .map_err(|_| format!("Invalid height: {}", parts[1]))?;

    if width <= 0.0 || height <= 0.0 {
        return Err("Width and height must be positive".to_string());
    }

    Ok(Vector2D::new(width, height))
}

impl Config {
    pub fn from_cli(cli: Cli) -> Self {
        let mut config = Config::default();
        config.move_type = cli.move_type;
        if let Some(size) = cli.size {
            config.size = size;
        }
        config.render_inactive = cli.render_inactive;
        config.no_fractional = cli.no_fractional;
        config.continuous_capture = cli.continuous;
        config.zoom_speed = cli.zoom_speed.max(0.001).min(1.0); // Clamp to reasonable range
        config.exit_delay_ms = cli.exit_delay.min(5000); // Max 5 seconds
        config
    }

    pub fn log_level(&self, cli: &Cli) -> log::LevelFilter {
        if cli.quiet {
            log::LevelFilter::Error
        } else if cli.verbose {
            log::LevelFilter::Trace
        } else {
            log::LevelFilter::Info
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_size() {
        assert_eq!(parse_size("300x150").unwrap(), Vector2D::new(300.0, 150.0));
        assert_eq!(parse_size("1920x1080").unwrap(), Vector2D::new(1920.0, 1080.0));
        assert!(parse_size("invalid").is_err());
        assert!(parse_size("300").is_err());
        assert!(parse_size("-300x150").is_err());
    }
}
