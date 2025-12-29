//! Configuration management and CLI argument parsing.
//!
//! This module handles all configuration options for remagnify, including
//! CLI argument parsing, validation, and default values.

use crate::utils::Vector2D;
use clap::{Parser, ValueEnum};

/// Magnifier movement mode.
///
/// Determines how the magnifying frame follows the cursor.
#[derive(Debug, Clone, Copy, ValueEnum, Default)]
pub enum MoveType {
    /// Magnifier moves relative to cursor movement (for precise positioning).
    Corner,
    /// Magnifier directly follows the cursor position (default).
    #[default]
    Cursor,
}

/// Application configuration.
///
/// Contains all validated configuration options for the magnifier.
/// Values are clamped to safe ranges during construction from CLI args.
#[derive(Debug, Clone)]
pub struct Config {
    #[allow(dead_code)]
    pub move_type: MoveType,
    pub size: Vector2D,
    #[allow(dead_code)]
    pub render_inactive: bool,
    #[allow(dead_code)]
    pub continuous_capture: bool,
    pub zoom_speed: f64,
    pub exit_delay_ms: u64,
    pub hide_cursor: bool,
    /// Fractional scale override (e.g., 1.5 for 150% scaling).
    /// If None, uses the integer scale from wl_output.
    pub scale: Option<f64>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            move_type: MoveType::Cursor,
            size: Vector2D::new(300.0, 150.0),
            render_inactive: false,
            continuous_capture: true,
            zoom_speed: 0.05, // Default zoom speed (5% per scroll notch)
            exit_delay_ms: 200, // Default 200ms delay before exit
            hide_cursor: true, // Hide cursor by default
            scale: None, // Auto-detect from wl_output
        }
    }
}

/// Command-line interface arguments.
///
/// Parsed using clap. All arguments are validated and converted to a Config.
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

    /// Show cursor (cursor is hidden by default)
    #[arg(long)]
    pub show_cursor: bool,

    /// Override monitor scale (e.g., 1.5 for 150% scaling).
    /// Use this for fractional scaling if auto-detection doesn't work.
    /// If not specified, uses the integer scale from wl_output.
    #[arg(long)]
    pub scale: Option<f64>,
}

/// Parse a size string in the format "WIDTHxHEIGHT".
///
/// # Arguments
///
/// * `s` - String in format "300x150" or similar
///
/// # Returns
///
/// * `Ok(Vector2D)` - Parsed size with positive dimensions
/// * `Err(String)` - If format is invalid or dimensions are negative/zero
///
/// # Examples
///
/// ```ignore
/// let size = parse_size("300x150")?;
/// assert_eq!(size, Vector2D::new(300.0, 150.0));
/// ```
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
    /// Create a Config from CLI arguments.
    ///
    /// Validates and clamps all values to safe ranges:
    /// - zoom_speed: clamped to 0.001..=1.0
    /// - exit_delay_ms: clamped to 0..=5000
    ///
    /// # Arguments
    ///
    /// * `cli` - Parsed command-line arguments
    ///
    /// # Returns
    ///
    /// A Config with validated values
    pub fn from_cli(cli: Cli) -> Self {
        // Validate scale if provided
        let scale = cli.scale.map(|s| {
            if s <= 0.0 {
                log::warn!("Scale must be positive, using default");
                None
            } else if s > 10.0 {
                log::warn!("Scale too high (max 10.0), clamping");
                Some(10.0)
            } else {
                Some(s)
            }
        }).flatten();

        Config {
            move_type: cli.move_type,
            size: cli.size.unwrap_or_else(|| Config::default().size),
            render_inactive: cli.render_inactive,
            continuous_capture: cli.continuous,
            zoom_speed: cli.zoom_speed.clamp(0.001, 1.0),
            exit_delay_ms: cli.exit_delay.min(5000),
            hide_cursor: !cli.show_cursor, // Invert: show_cursor flag disables hiding
            scale,
        }
    }

    #[allow(dead_code)]
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

    #[test]
    fn test_config_from_cli() {
        let cli = Cli {
            move_type: MoveType::Corner,
            size: Some(Vector2D::new(400.0, 200.0)),
            render_inactive: true,
            continuous: false,
            zoom_speed: 0.1,
            exit_delay: 500,
            quiet: false,
            verbose: false,
            show_cursor: false,
            scale: None,
        };

        let config = Config::from_cli(cli);
        assert_eq!(config.size.x, 400.0);
        assert_eq!(config.size.y, 200.0);
        assert_eq!(config.zoom_speed, 0.1);
        assert_eq!(config.exit_delay_ms, 500);
        assert_eq!(config.hide_cursor, true); // Default: cursor hidden
        assert_eq!(config.scale, None);
    }

    #[test]
    fn test_config_zoom_speed_clamping() {
        // Test that zoom speed is clamped to valid range
        let cli_too_low = Cli {
            move_type: MoveType::Cursor,
            size: None,
            render_inactive: false,
            continuous: true,
            zoom_speed: -0.5, // Invalid
            exit_delay: 200,
            quiet: false,
            verbose: false,
            show_cursor: false,
            scale: None,
        };

        let config = Config::from_cli(cli_too_low);
        assert!(config.zoom_speed >= 0.001); // Should be clamped to minimum

        let cli_too_high = Cli {
            move_type: MoveType::Cursor,
            size: None,
            render_inactive: false,
            continuous: true,
            zoom_speed: 5.0, // Invalid
            exit_delay: 200,
            quiet: false,
            verbose: false,
            show_cursor: false,
            scale: None,
        };

        let config = Config::from_cli(cli_too_high);
        assert!(config.zoom_speed <= 1.0); // Should be clamped to maximum
    }

    #[test]
    fn test_config_exit_delay_clamping() {
        // Test that exit delay is clamped to maximum
        let cli = Cli {
            move_type: MoveType::Cursor,
            size: None,
            render_inactive: false,
            continuous: true,
            zoom_speed: 0.05,
            exit_delay: 10000, // Too high
            quiet: false,
            verbose: false,
            show_cursor: false,
            scale: None,
        };

        let config = Config::from_cli(cli);
        assert!(config.exit_delay_ms <= 5000); // Should be clamped to 5000ms max
    }

    #[test]
    fn test_cursor_hiding_config() {
        // Test that cursor is hidden by default
        let cli_default = Cli {
            move_type: MoveType::Cursor,
            size: None,
            render_inactive: false,
            continuous: true,
            zoom_speed: 0.05,
            exit_delay: 200,
            quiet: false,
            verbose: false,
            show_cursor: false, // Default: don't show cursor
            scale: None,
        };

        let config = Config::from_cli(cli_default);
        assert_eq!(config.hide_cursor, true); // Cursor should be hidden

        // Test that --show-cursor flag works
        let cli_show = Cli {
            move_type: MoveType::Cursor,
            size: None,
            render_inactive: false,
            continuous: true,
            zoom_speed: 0.05,
            exit_delay: 200,
            quiet: false,
            verbose: false,
            show_cursor: true, // Explicitly show cursor
            scale: None,
        };

        let config = Config::from_cli(cli_show);
        assert_eq!(config.hide_cursor, false); // Cursor should be visible
    }

    #[test]
    fn test_scale_validation() {
        // Test valid scale
        let cli_valid = Cli {
            move_type: MoveType::Cursor,
            size: None,
            render_inactive: false,
            continuous: true,
            zoom_speed: 0.05,
            exit_delay: 200,
            quiet: false,
            verbose: false,
            show_cursor: false,
            scale: Some(1.5),
        };

        let config = Config::from_cli(cli_valid);
        assert_eq!(config.scale, Some(1.5));

        // Test scale clamping to maximum
        let cli_too_high = Cli {
            move_type: MoveType::Cursor,
            size: None,
            render_inactive: false,
            continuous: true,
            zoom_speed: 0.05,
            exit_delay: 200,
            quiet: false,
            verbose: false,
            show_cursor: false,
            scale: Some(15.0), // Too high
        };

        let config = Config::from_cli(cli_too_high);
        assert_eq!(config.scale, Some(10.0)); // Should be clamped to 10.0

        // Test invalid scale (negative)
        let cli_negative = Cli {
            move_type: MoveType::Cursor,
            size: None,
            render_inactive: false,
            continuous: true,
            zoom_speed: 0.05,
            exit_delay: 200,
            quiet: false,
            verbose: false,
            show_cursor: false,
            scale: Some(-1.5), // Invalid
        };

        let config = Config::from_cli(cli_negative);
        assert_eq!(config.scale, None); // Should be rejected
    }
}
